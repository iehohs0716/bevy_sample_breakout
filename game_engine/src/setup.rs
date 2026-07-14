//! 起動時に一度だけ走るセットアップ system。カメラ・背景・音・パドル・ボール・
//! スコア表示・壁・ブロックを spawn する。

use bevy::prelude::*;

use crate::components::{
    Ball, Collider, CollisionSound, Paddle, ScoreboardUi, Velocity, Wall, WallLocation,
};
use crate::config::{
    BACKGROUND_IMAGE_PATH, BACKGROUND_SIZE, BALL_COLOR, BALL_DIAMETER, BALL_SPEED,
    BALL_STARTING_POSITION, BOTTOM_WALL, BRICK_SIZE, GAP_BETWEEN_BRICKS,
    GAP_BETWEEN_BRICKS_AND_CEILING, GAP_BETWEEN_BRICKS_AND_SIDES, GAP_BETWEEN_PADDLE_AND_BRICKS,
    GAP_BETWEEN_PADDLE_AND_FLOOR, INITIAL_BALL_DIRECTION, LEFT_WALL, PADDLE_COLOR, PADDLE_SIZE,
    RIGHT_WALL, SCORE_COLOR, SCOREBOARD_FONT_SIZE, SCOREBOARD_TEXT_PADDING, TEXT_COLOR, TOP_WALL,
};
use crate::injection::{BackgroundOverride, BrickImagesOverride, BrickLayoutOverride};
use crate::rendering::{brick_image_for, contain_fit, spawn_brick};

// Add the game's entities to our world
pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut background_override: ResMut<BackgroundOverride>,
    mut brick_layout_override: ResMut<BrickLayoutOverride>,
    mut brick_images_override: ResMut<BrickImagesOverride>,
    asset_server: Res<AssetServer>,
) {
    // Camera
    commands.spawn(Camera2d);

    // Background image
    // React（JS）が背景画像バイト列を渡していれば、それをデコード済みの `Image` として
    // `Assets<Image>` に登録してハンドルを得る。渡されていなければ `BACKGROUND_IMAGE_PATH`
    // のデフォルト画像を AssetServer 経由でロードする。
    // アスペクト比は変えず（引き伸ばさず）、アリーナに contain フィットさせて中央に置く。
    // 比率が合わない余白は画面クリア色（黒）が見える。
    // z を負にして他の要素（壁・ブロック・ボール）より後ろに配置する。
    // 注: 寸法が分かるのは React が Image を渡したときだけ。AssetServer ロード時は寸法が
    // 起動時点で未確定なので、従来どおりアリーナ全体に引き伸ばす。
    let (background_handle, background_size) = match background_override.0.take() {
        Some(image) => {
            let image_size = Vec2::new(image.width() as f32, image.height() as f32);
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
        Transform::from_xyz(0.0, 0.0, -10.0),
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
    // React（JS）が渡したブロック用画像（2 種類想定）を `Assets<Image>` に登録し、
    // 「ハンドル + 元のピクセル寸法」の列にしておく。寸法は各ブロックが画像のどの領域を
    // 切り出すか（brick_image_rect）に使う。空なら単色ブロックにフォールバックする。
    let brick_images: Vec<(Handle<Image>, Vec2)> = brick_images_override
        .0
        .drain(..)
        .map(|image| {
            let size = Vec2::new(image.width() as f32, image.height() as f32);
            (images.add(image), size)
        })
        .collect();

    // React（JS）が初期ブロック配置を渡していれば、それを使って spawn する。
    // 渡されていなければ、従来どおりアリーナを敷き詰めるデフォルト配置を計算して使う。
    // どちらの場合も、生成順のインデックスで画像を交互（折り返し）に割り当てる。
    if let Some(layout) = brick_layout_override.0.take() {
        for (index, position) in layout.positions.iter().enumerate() {
            let image = brick_image_for(&brick_images, index);
            spawn_brick(&mut commands, *position, layout.cell_size, image);
        }
        return;
    }

    let total_width_of_bricks = (RIGHT_WALL - LEFT_WALL) - 2. * GAP_BETWEEN_BRICKS_AND_SIDES;
    let bottom_edge_of_bricks = paddle_y + GAP_BETWEEN_PADDLE_AND_BRICKS;
    let total_height_of_bricks = TOP_WALL - bottom_edge_of_bricks - GAP_BETWEEN_BRICKS_AND_CEILING;

    assert!(total_width_of_bricks > 0.0);
    assert!(total_height_of_bricks > 0.0);

    // Given the space available, compute how many rows and columns of bricks we can fit
    let n_columns = (total_width_of_bricks / (BRICK_SIZE.x + GAP_BETWEEN_BRICKS)).floor() as usize;
    let n_rows = (total_height_of_bricks / (BRICK_SIZE.y + GAP_BETWEEN_BRICKS)).floor() as usize;
    let n_vertical_gaps = n_columns - 1;

    // Because we need to round the number of columns,
    // the space on the top and sides of the bricks only captures a lower bound, not an exact value
    let center_of_bricks = (LEFT_WALL + RIGHT_WALL) / 2.0;
    let left_edge_of_bricks = center_of_bricks
        // Space taken up by the bricks
        - (n_columns as f32 / 2.0 * BRICK_SIZE.x)
        // Space taken up by the gaps
        - n_vertical_gaps as f32 / 2.0 * GAP_BETWEEN_BRICKS;

    // In Bevy, the `translation` of an entity describes the center point,
    // not its bottom-left corner
    let offset_x = left_edge_of_bricks + BRICK_SIZE.x / 2.;
    let offset_y = bottom_edge_of_bricks + BRICK_SIZE.y / 2.;

    let mut brick_index = 0;
    for row in 0..n_rows {
        for column in 0..n_columns {
            let brick_position = Vec2::new(
                offset_x + column as f32 * (BRICK_SIZE.x + GAP_BETWEEN_BRICKS),
                offset_y + row as f32 * (BRICK_SIZE.y + GAP_BETWEEN_BRICKS),
            );

            let image = brick_image_for(&brick_images, brick_index);
            spawn_brick(&mut commands, brick_position, BRICK_SIZE, image);
            brick_index += 1;
        }
    }
}
