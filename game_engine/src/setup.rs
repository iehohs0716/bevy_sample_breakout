//! 起動時に一度だけ走るセットアップ system。カメラ・背景・音・パドル・ボール・
//! スコア表示・壁・ブロックを spawn する。

use bevy::prelude::*;

use crate::components::{
    Ball, Collider, CollisionSound, DeathZone, GameAssets, LivesUi, Paddle, ScoreboardUi, Velocity,
    Wall, WallLocation,
};
use crate::config::{
    BACKGROUND_IMAGE_PATH, BACKGROUND_SIZE, BALL_COLOR, BALL_DIAMETER, BALL_STARTING_POSITION,
    BOTTOM_WALL, GAP_BETWEEN_PADDLE_AND_FLOOR, PADDLE_COLOR, PADDLE_SIZE, SCORE_COLOR,
    SCOREBOARD_FONT_SIZE, SCOREBOARD_TEXT_PADDING, TEXT_COLOR,
};
use crate::injection::{
    default_brick_layout, BackgroundOverride, BrickImageOverride, BrickLayoutOverride,
};
use crate::rendering::{contain_fit, spawn_brick};

// Add the game's entities to our world
pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut background_override: ResMut<BackgroundOverride>,
    mut brick_layout_override: ResMut<BrickLayoutOverride>,
    mut brick_image_override: ResMut<BrickImageOverride>,
    asset_server: Res<AssetServer>,
) {
    // Camera
    commands.spawn(Camera2d);

    // Background image
    // background_override.0.take()の成否によって挙動を変更
    // 成功 -> `Assets<Image>` に登録してハンドル(画像の参照)を取得する。
    // 失敗 -> 既存のリソースを使う
    let (background_handle, background_size) = match background_override.0.take() {
        Some(image) => {
            let image_size = Vec2::new(image.width() as f32, image.height() as f32);
            // アスペクト比を変えないようにサイズを補正
            (images.add(image), contain_fit(image_size, BACKGROUND_SIZE)) 
        }
        None => (asset_server.load(BACKGROUND_IMAGE_PATH), BACKGROUND_SIZE),
    };
    commands.spawn((
        Sprite {
            image: background_handle,
            custom_size: Some(background_size),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -10.0),     // z を負にして他の要素（壁・ブロック・ボール）より後ろに配置する。
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
        Mesh2d(meshes.add(Circle::default())),
        MeshMaterial2d(materials.add(BALL_COLOR)),
        Transform::from_translation(BALL_STARTING_POSITION)
            .with_scale(Vec2::splat(BALL_DIAMETER).extend(1.)),
        Ball,
        Velocity(Vec2::ZERO),
    ));

    // Scoreboard + Lives
    // 画面左上に横並びで「Lives: N   Score: M」と並べる（Lives が Score の左側）。
    // 横並び（flex Row）のコンテナに、Lives → Score の順で子として置く。
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: SCOREBOARD_TEXT_PADDING,
            left: SCOREBOARD_TEXT_PADDING,
            column_gap: Val::Px(20.0),
            ..default()
        },
        children![
            (
                Text::new("Lives: "),
                TextFont {
                    font_size: SCOREBOARD_FONT_SIZE,
                    ..default()
                },
                TextColor(TEXT_COLOR),
                LivesUi,
                children![(
                    TextSpan::default(),
                    TextFont {
                        font_size: SCOREBOARD_FONT_SIZE,
                        ..default()
                    },
                    TextColor(SCORE_COLOR),
                )],
            ),
            (
                Text::new("Score: "),
                TextFont {
                    font_size: SCOREBOARD_FONT_SIZE,
                    ..default()
                },
                TextColor(TEXT_COLOR),
                ScoreboardUi,
                children![(
                    TextSpan::default(),
                    TextFont {
                        font_size: SCOREBOARD_FONT_SIZE,
                        ..default()
                    },
                    TextColor(SCORE_COLOR),
                )],
            ),
        ],
    ));

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

    // ブロック配置を確定する。React（JS）が配置を渡していればそれを、無ければアリーナを敷き詰める
    // デフォルト配置を使う。
    let brick_layout = brick_layout_override
        .0
        .take()
        .unwrap_or_else(|| default_brick_layout(paddle_y));

    // 初期盤面のブロックを spawn する。初期状態 GameStart の OnEnter（reset_game）は Bevy の仕様上
    // Startup より前に 1 回走り、その時点では GameAssets が未生成で何もしない。よって初回の配置は
    // setup が担い、reset_game は 2 回目以降（GameOver→GameStart の再スタート）で作り直す。
    for position in &brick_layout.positions {
        spawn_brick(&mut commands, *position, brick_layout.cell_size, brick_image.clone());
    }

    // 再スタートでブロックを配置し直せるよう、確定した配置と画像を保持しておく。
    commands.insert_resource(GameAssets {
        brick_layout,
        brick_image,
    });
}
