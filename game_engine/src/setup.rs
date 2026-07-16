//! 起動時に一度だけ走るセットアップ system。カメラ・背景・音・パドル・ボール・
//! スコア表示・壁・ブロックを spawn する。

use bevy::prelude::*;

use crate::components::{
    Ball, Collider, CollisionSound, Paddle, ScoreboardUi, Velocity, Wall, WallLocation,
};
use crate::config::{
    BACKGROUND_IMAGE_PATH, BACKGROUND_SIZE, BALL_COLOR, BALL_DIAMETER, BALL_SPEED,
    BALL_STARTING_POSITION, BOTTOM_WALL, GAP_BETWEEN_PADDLE_AND_FLOOR, INITIAL_BALL_DIRECTION,
    PADDLE_COLOR, PADDLE_SIZE, SCORE_COLOR, SCOREBOARD_FONT_SIZE, SCOREBOARD_TEXT_PADDING,
    TEXT_COLOR,
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
    commands.spawn((
        Mesh2d(meshes.add(Circle::default())),
        MeshMaterial2d(materials.add(BALL_COLOR)),
        Transform::from_translation(BALL_STARTING_POSITION)
            .with_scale(Vec2::splat(BALL_DIAMETER).extend(1.)),
        Ball,
        Velocity(INITIAL_BALL_DIRECTION.normalize() * BALL_SPEED),
    ));

    // Scoreboard
    commands.spawn((
        Text::new("Score: "),
        TextFont {
            font_size: SCOREBOARD_FONT_SIZE,
            ..default()
        },
        TextColor(TEXT_COLOR),
        ScoreboardUi,
        Node {
            position_type: PositionType::Absolute,
            top: SCOREBOARD_TEXT_PADDING,
            left: SCOREBOARD_TEXT_PADDING,
            ..default()
        },
        children![(
            TextSpan::default(),
            TextFont {
                font_size: SCOREBOARD_FONT_SIZE,
                ..default()
            },
            TextColor(SCORE_COLOR),
        )],
    ));

    // Walls
    commands.spawn(Wall::new(WallLocation::Left));
    commands.spawn(Wall::new(WallLocation::Right));
    commands.spawn(Wall::new(WallLocation::Bottom));
    commands.spawn(Wall::new(WallLocation::Top));

    // Bricks
    // React（JS）が渡したブロック用画像を `Assets<Image>` に登録し、「ハンドル + 元のピクセル寸法」
    // にしておく。寸法は各ブロックが画像のどの領域を切り出すか（brick_image_rect）に使う。全ブロックが
    // 同じ画像を共有し、各自の位置に対応する領域を表示する。`None` なら単色ブロックにフォールバック。
    let brick_image: Option<(Handle<Image>, Vec2)> = brick_image_override.0.take().map(|image| {
        let size = Vec2::new(image.width() as f32, image.height() as f32);
        (images.add(image), size)
    });

    // ブロック配置は「レイアウトを取得 → 各位置に spawn」の一本道。React（JS）が配置を
    // 渡していればそれを、無ければアリーナを敷き詰めるデフォルト配置を使う。どちらも同じ
    // `BrickLayout` なので、以降の spawn ロジックを分岐させる必要はない。
    let layout = brick_layout_override
        .0
        .take()
        .unwrap_or_else(|| default_brick_layout(paddle_y));
    for position in &layout.positions {
        spawn_brick(&mut commands, *position, layout.cell_size, brick_image.clone());
    }
}
