//! 起動時に一度だけ走るセットアップ system。カメラ・背景・音・パドル・ボール・
//! スコア表示・壁・ブロックを spawn する。

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::common::BrickAssets;
use crate::components::{
    Ball, Collider, CollisionSound, DeathZone, GameAssets, Paddle, Velocity,
    Wall, WallLocation,
};
use crate::config::{
    BACKGROUND_IMAGE_PATH, BACKGROUND_SIZE, BALL_COLOR, BALL_DIAMETER, BALL_STARTING_POSITION,
    BOTTOM_WALL, BRICK_SIZE, GAP_BETWEEN_PADDLE_AND_FLOOR, PADDLE_COLOR, PADDLE_SIZE, TOP_WALL,
};
use crate::common::brick::layout::{default_brick_layout, diff_brick_layout};
use crate::injection::{
    BackgroundOverride, BrickImageOverride, BrickLayoutOverride, injected_cell_size,
};
use crate::util::contain_fit;

// Add the game's entities to our world
pub fn setup(
    mut commands: Commands,
    mut brick_assets: BrickAssets,
    mut images: ResMut<Assets<Image>>,
    mut background_override: ResMut<BackgroundOverride>,
    mut brick_layout_override: ResMut<BrickLayoutOverride>,
    mut brick_image_override: ResMut<BrickImageOverride>,
    asset_server: Res<AssetServer>,
) {
    // Camera
    commands.spawn(Camera2d);

    // セルサイズは `bricks`（明示配置）が無くても JS の `cellSize` 指定を使う（2・3 経路共通。 無指定なら `BRICK_SIZE`）。
    let cell_size = injected_cell_size().unwrap_or(BRICK_SIZE);

    // Background image
    // background_override.0.take()の成否によって挙動を変更
    // 成功 -> `Assets<Image>` に登録してハンドル(画像の参照)を取得する。
    // 失敗 -> 既存のリソースを使う
    // `background_was_overridden` はブロックの画像差分自動配置（後述）の発火条件判定に使う。
    // `background_area` は画像の生ピクセル寸法（0,0起点の矩形）。`Sprite.rect` に渡す用で、
    // デフォルト背景（`asset_server.load`）は非同期ロードでこの時点ではまだデコードが
    // 終わっていないため `None`（サイズ不明）になる。
    let background_was_overridden = background_override.0.is_some();
    let (background_handle, background_size, background_area) = match background_override.0.take() {
        Some(image) => {
            let image_size = Vec2::new(image.width() as f32, image.height() as f32);
            let (fixed_image_size, scale) = contain_fit(image_size, BACKGROUND_SIZE);
            let remainder_y = fixed_image_size.y % cell_size.y;
            let cropped_display_size =
                Vec2::new(fixed_image_size.x, fixed_image_size.y - remainder_y);
            let crop_texture_pixels_top = remainder_y / scale;

            (
                images.add(image),
                cropped_display_size,
                Some(Rect {
                    min: Vec2::new(0.0, crop_texture_pixels_top),
                    max: image_size,
                }),
            )
        }
        None => (
            asset_server.load(BACKGROUND_IMAGE_PATH),
            BACKGROUND_SIZE,
            None,
        ),
    };
    commands.spawn((
        Sprite {
            // ブロックの画像差分自動配置（後述）でも同じ画像を参照するため、ハンドルは
            // clone して渡す（Handle は Arc ベースで clone は軽量）。
            image: background_handle.clone(),
            custom_size: Some(background_size),
            // 画像全体を使うだけなので描画結果は `rect: None`（デフォルト）と変わらないが、
            // 生の画像サイズが分かっている場合（JS からの注入がある場合）はそれを明示する。
            // 未確定（`None`）のときに適当な `Rect` を補って渡すと UV が壊れるため、
            // 分からないときは必ず `None` のままにすること。
            rect: background_area,
            ..default()
        },
        // 内接表示は水平中央・垂直上寄せ（`util::inscribed_source_rect` と同じ規約）。
        // アンカーを上端中央にし、原点を Y=TOP_WALL（アリーナ天井）に置くことで、
        // 画像の上端をアリーナ天井に揃える。横長画像で余った高さの余白は下側だけに寄る。
        Anchor::TOP_CENTER,
        // z を負にして他の要素（壁・ブロック・ボール）より後ろに配置する。
        Transform::from_xyz(0.0, TOP_WALL, -10.0),
    ));

    // Sound
    let ball_collision_sound = asset_server.load("sounds/breakout_collision.ogg");
    commands.insert_resource(CollisionSound(ball_collision_sound));

    // Paddle
    let paddle_y = BOTTOM_WALL + GAP_BETWEEN_PADDLE_AND_FLOOR;
    commands.spawn((
        Sprite::from_color(PADDLE_COLOR, Vec2::ONE),
        Transform {
            translation: Vec3::new(0.0, paddle_y, 0.0),
            scale: PADDLE_SIZE.extend(1.0),
            ..default()
        },
        Paddle,
        Collider,
    ));

    // Ball
    // 初速は 0（静止）。GameStart 中の左クリックで発射する（`launch_ball_on_click`）。
    // 盤面の初期化（位置・速度リセット）は `OnEnter(GameStart)` の `reset_game` が担う。
    commands.spawn((
        Mesh2d(brick_assets.meshes.add(Circle::default())),
        MeshMaterial2d(brick_assets.materials.add(BALL_COLOR)),
        Transform::from_translation(BALL_STARTING_POSITION)
            .with_scale(Vec2::splat(BALL_DIAMETER).extend(1.)),
        Ball,
        Velocity(Vec2::ZERO),
    ));

    // Scoreboard + Lives
    crate::common::spawn_scoreboard(&mut commands);

    // Walls
    // 下端は反射する壁ではなく DeathZone（触れるとライフが減る領域）にする。
    commands.spawn(Wall::new(WallLocation::Left));
    commands.spawn(Wall::new(WallLocation::Right));
    commands.spawn(Wall::new(WallLocation::Top));
    commands.spawn(DeathZone::new());

    // Bricks
    // React（JS）が渡したブロック用画像を `Assets<Image>` に登録し、「ハンドル + 元のピクセル寸法」
    // にしておく。寸法は各ブロックが画像のどの領域を切り出すか（brick_image_rect）に使う。全ブロックが
    // 同じ画像を共有し、各自の位置に対応する領域を表示する。`None` なら単色ブロックにフォールバック。
    let brick_image: Option<(Handle<Image>, Vec2)> = brick_image_override.0.take().map(|image| {
        let size = Vec2::new(image.width() as f32, image.height() as f32);
        (images.add(image), size)
    });

    // ブロック配置を確定する。優先順位は
    // 1. React（JS）が明示指定した配置（`brick_layout_override`）
    // 2. 背景画像・ブロック画像が両方とも JS 由来なら、2 画像の差分から自動生成
    //    （`diff_brick_layout`。差分が 1 つも無ければ 3 にフォールバック）
    // 3. アリーナを敷き詰めるデフォルト配置
    // 差分計算には生ピクセルが要るが、`Assets<Image>` 登録後でも `images.get(&handle)` で
    // 参照を取り直せる（デコード時に CPU 側データも保持する設定にしているため）。
    let brick_layout = brick_layout_override.0.take().unwrap_or_else(|| {
        let diffed = background_was_overridden
            .then(|| {
                brick_image
                    .as_ref()
                    .and_then(|(handle, _)| images.get(handle))
                    .zip(images.get(&background_handle))
                    .and_then(|(brick_img, background_img)| {
                        diff_brick_layout(background_img, brick_img, paddle_y, cell_size)
                    })
            })
            .flatten();
        diffed.unwrap_or_else(|| default_brick_layout(paddle_y, cell_size))
    });

    // 各ブロックを配置座標に spawn する（＝起動時の初期盤面）。初回配置は setup が担い、
    // 敗北後の再スタート（GameOver→GameRestart）でのブロック作り直しは reset_game が担う。
    for (position, cell) in brick_layout.positions.iter().zip(&brick_layout.cells) {
        crate::common::spawn_brick(
            &mut commands,
            &mut brick_assets,
            *position,
            brick_layout.cell_size,
            *cell,
            brick_image.clone(),
        );
    }

    // 再スタートでブロックを配置し直せるよう、確定した配置と画像を保持しておく。
    commands.insert_resource(GameAssets {
        brick_layout,
        brick_image,
    });
}
