//! A simplified implementation of the classic game "Breakout".
//!
//! Demonstrates Bevy's stepping capabilities if compiled with the `bevy_debug_stepping` feature.

use bevy::{
    math::bounding::{Aabb2d, BoundingCircle, BoundingVolume, IntersectsVolume},
    prelude::*,
};

// These constants are defined in `Transform` units.
// Using the default 2D camera they correspond 1:1 with screen pixels.
const PADDLE_SIZE: Vec2 = Vec2::new(120.0, 20.0);
const GAP_BETWEEN_PADDLE_AND_FLOOR: f32 = 60.0;
const PADDLE_SPEED: f32 = 500.0;
// How close can the paddle get to the wall
const PADDLE_PADDING: f32 = 10.0;

// We set the z-value of the ball to 1 so it renders on top in the case of overlapping sprites.
const BALL_STARTING_POSITION: Vec3 = Vec3::new(0.0, -50.0, 1.0);
const BALL_DIAMETER: f32 = 30.;
const BALL_SPEED: f32 = 400.0;
const INITIAL_BALL_DIRECTION: Vec2 = Vec2::new(0.5, -0.5);

const WALL_THICKNESS: f32 = 10.0;
// x coordinates
const LEFT_WALL: f32 = -450.;
const RIGHT_WALL: f32 = 450.;
// y coordinates
const BOTTOM_WALL: f32 = -300.;
const TOP_WALL: f32 = 300.;

const BRICK_SIZE: Vec2 = Vec2::new(50., 30.);
// These values are exact
const GAP_BETWEEN_PADDLE_AND_BRICKS: f32 = 270.0;
const GAP_BETWEEN_BRICKS: f32 = 0.0;
// These values are lower bounds, as the number of bricks is computed
const GAP_BETWEEN_BRICKS_AND_CEILING: f32 = 0.0;
const GAP_BETWEEN_BRICKS_AND_SIDES: f32 = 0.0;

const SCOREBOARD_FONT_SIZE: FontSize = FontSize::Px(33.0);
const SCOREBOARD_TEXT_PADDING: Val = Val::Px(5.0);

// 背景画像のデフォルトパス（AssetServer 基準。Web では `/assets/` 配下、ネイティブでは
// `game_engine/assets/` 配下を指す）。
//
// Web ビルドでは、React 側が `window.__BREAKOUT_CONFIG__.backgroundBytes` に
// fetch 済みの画像バイト列（Uint8Array）を載せていれば、それを優先して背景に使う。
// これにより「アプリのコードは1つ」のまま、サービスごとに（S3 等の任意 URL の画像でも）
// 背景だけを差し替えられる。React 側が bytes を渡さない場合はこのデフォルトにフォールバックする。
const BACKGROUND_IMAGE_PATH: &str = "backgrounds/background.png";
// 背景スプライトの表示サイズ（アリーナ全体を覆う）。
const BACKGROUND_SIZE: Vec2 = Vec2::new(RIGHT_WALL - LEFT_WALL, TOP_WALL - BOTTOM_WALL);

const PADDLE_COLOR: Color = Color::srgb(0.3, 0.3, 0.7);
const BALL_COLOR: Color = Color::srgb(1.0, 0.5, 0.5);
const BRICK_COLOR: Color = Color::srgb(0.5, 0.5, 1.0);
const WALL_COLOR: Color = Color::srgb(0.8, 0.8, 0.8);
const TEXT_COLOR: Color = Color::srgb(0.5, 0.5, 1.0);
const SCORE_COLOR: Color = Color::srgb(1.0, 0.5, 0.5);

// React（JS）から渡された背景画像を一時的に保持する Resource。
// `setup` で取り出して `Assets<Image>` に登録し、背景スプライトに使う。
// `None` の場合は `BACKGROUND_IMAGE_PATH` のデフォルト画像にフォールバックする。
#[derive(Resource, Default)]
struct BackgroundOverride(Option<Image>);

/// Web ビルド専用。`window.__BREAKOUT_CONFIG__.backgroundBytes`（React が fetch した
/// 画像バイト列 = Uint8Array）を読み、`Image` にデコードして返す。
/// 設定が無い / 読めない / デコード失敗の場合は `None`（デフォルト背景にフォールバック）。
#[cfg(target_arch = "wasm32")]
fn injected_background_image() -> Option<Image> {
    use bevy::{
        asset::RenderAssetUsages,
        image::{CompressedImageFormats, ImageSampler, ImageType},
    };
    use wasm_bindgen::{JsCast, JsValue};

    let window = web_sys::window()?;
    let config = js_sys::Reflect::get(&window, &JsValue::from_str("__BREAKOUT_CONFIG__")).ok()?;
    if config.is_undefined() || config.is_null() {
        return None;
    }

    let bytes_val = js_sys::Reflect::get(&config, &JsValue::from_str("backgroundBytes")).ok()?;
    let bytes = bytes_val.dyn_into::<js_sys::Uint8Array>().ok()?.to_vec();
    if bytes.is_empty() {
        return None;
    }

    // 画像フォーマットは MIME で受け取れれば使い、無ければ拡張子 png とみなす。
    let mime = js_sys::Reflect::get(&config, &JsValue::from_str("backgroundMime"))
        .ok()
        .and_then(|v| v.as_string());
    let image_type = match mime.as_deref() {
        Some(m) if !m.is_empty() => ImageType::MimeType(m),
        _ => ImageType::Extension("png"),
    };

    match Image::from_buffer(
        &bytes,
        image_type,
        CompressedImageFormats::NONE,
        true,
        ImageSampler::Default,
        RenderAssetUsages::default(),
    ) {
        Ok(image) => Some(image),
        Err(err) => {
            warn!("背景画像のデコードに失敗しました。デフォルト背景を使用します: {err}");
            None
        }
    }
}

/// ネイティブビルドでは JS からの注入は無い（常にデフォルト背景を使う）。
#[cfg(not(target_arch = "wasm32"))]
fn injected_background_image() -> Option<Image> {
    None
}

// React（JS）から渡された「初期ブロック配置」。座標は Bevy ワールド座標
// （中心原点・y 上向き・1 単位 = 1px。アリーナは x∈[LEFT_WALL, RIGHT_WALL],
// y∈[BOTTOM_WALL, TOP_WALL]）で、各ブロックの *中心* 位置を表す。
// `cell_size` は全ブロック共通のセルの大きさ（幅・高さ）。
struct BrickLayout {
    positions: Vec<Vec2>,
    cell_size: Vec2,
}

// React（JS）から渡された初期ブロック配置を一時的に保持する Resource。
// `setup` で取り出してブロックを spawn する。`None` の場合は従来どおり
// アリーナを敷き詰めるデフォルト配置にフォールバックする。
#[derive(Resource, Default)]
struct BrickLayoutOverride(Option<BrickLayout>);

/// Web ビルド専用。`window.__BREAKOUT_CONFIG__.bricks`（`[{x, y}, ...]` の配列）と
/// `.cellSize`（`{width, height}`）を読み、初期ブロック配置として返す。
/// - `bricks` が無い / 空 / 各要素に x,y が無い場合は `None`（デフォルト配置にフォールバック）。
/// - `cellSize` が無い / 不正な場合はデフォルトの `BRICK_SIZE` を使う。
#[cfg(target_arch = "wasm32")]
fn injected_brick_layout() -> Option<BrickLayout> {
    use wasm_bindgen::{JsCast, JsValue};

    let window = web_sys::window()?;
    let config = js_sys::Reflect::get(&window, &JsValue::from_str("__BREAKOUT_CONFIG__")).ok()?;
    if config.is_undefined() || config.is_null() {
        return None;
    }

    let bricks_val = js_sys::Reflect::get(&config, &JsValue::from_str("bricks")).ok()?;
    let bricks_arr = bricks_val.dyn_into::<js_sys::Array>().ok()?;
    if bricks_arr.length() == 0 {
        return None;
    }

    let mut positions = Vec::with_capacity(bricks_arr.length() as usize);
    for i in 0..bricks_arr.length() {
        let brick = bricks_arr.get(i);
        let x = js_sys::Reflect::get(&brick, &JsValue::from_str("x"))
            .ok()
            .and_then(|v| v.as_f64());
        let y = js_sys::Reflect::get(&brick, &JsValue::from_str("y"))
            .ok()
            .and_then(|v| v.as_f64());
        match (x, y) {
            (Some(x), Some(y)) => positions.push(Vec2::new(x as f32, y as f32)),
            _ => warn!("ブロック配置の要素 {i} に数値の x/y が無いためスキップします"),
        }
    }
    if positions.is_empty() {
        return None;
    }

    // セルの大きさ。指定が無い / 不正な場合はデフォルトの BRICK_SIZE にフォールバック。
    let cell_size = js_sys::Reflect::get(&config, &JsValue::from_str("cellSize"))
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null())
        .and_then(|cell| {
            let w = js_sys::Reflect::get(&cell, &JsValue::from_str("width"))
                .ok()
                .and_then(|v| v.as_f64());
            let h = js_sys::Reflect::get(&cell, &JsValue::from_str("height"))
                .ok()
                .and_then(|v| v.as_f64());
            match (w, h) {
                (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some(Vec2::new(w as f32, h as f32)),
                _ => None,
            }
        })
        .unwrap_or(BRICK_SIZE);

    Some(BrickLayout {
        positions,
        cell_size,
    })
}

/// ネイティブビルドでは JS からの注入は無い（常にデフォルト配置を使う）。
#[cfg(not(target_arch = "wasm32"))]
fn injected_brick_layout() -> Option<BrickLayout> {
    None
}

// React（JS）から渡された「ブロック用の画像（2 種類想定）」を一時的に保持する Resource。
// `setup` で `Assets<Image>` に登録してハンドル列に変換し、各ブロックへ順番に割り当てる。
// 空の場合は従来どおり `BRICK_COLOR` の単色ブロックにフォールバックする。
#[derive(Resource, Default)]
struct BrickImagesOverride(Vec<Image>);

/// Web ビルド専用。`window.__BREAKOUT_CONFIG__.brickImages`
/// （`[{ bytes: Uint8Array, mime?: string }, ...]` の配列）を読み、
/// デコード済みの `Image` 列として返す。ブロックにはこの配列を先頭から順に
/// （個数を超えたら折り返して）割り当てる。
/// 設定が無い / 空 / 全てデコード失敗の場合は空 Vec（単色ブロックにフォールバック）。
#[cfg(target_arch = "wasm32")]
fn injected_brick_images() -> Vec<Image> {
    use bevy::{
        asset::RenderAssetUsages,
        image::{CompressedImageFormats, ImageSampler, ImageType},
    };
    use wasm_bindgen::{JsCast, JsValue};

    let mut result = Vec::new();

    let Some(window) = web_sys::window() else {
        return result;
    };
    let Ok(config) = js_sys::Reflect::get(&window, &JsValue::from_str("__BREAKOUT_CONFIG__"))
    else {
        return result;
    };
    if config.is_undefined() || config.is_null() {
        return result;
    }
    let Ok(images_val) = js_sys::Reflect::get(&config, &JsValue::from_str("brickImages")) else {
        return result;
    };
    let Ok(images_arr) = images_val.dyn_into::<js_sys::Array>() else {
        return result;
    };

    for i in 0..images_arr.length() {
        let entry = images_arr.get(i);
        let bytes = match js_sys::Reflect::get(&entry, &JsValue::from_str("bytes"))
            .ok()
            .and_then(|v| v.dyn_into::<js_sys::Uint8Array>().ok())
        {
            Some(arr) => arr.to_vec(),
            None => {
                warn!("ブロック画像の要素 {i} に bytes(Uint8Array) が無いためスキップします");
                continue;
            }
        };
        if bytes.is_empty() {
            continue;
        }

        // 画像フォーマットは MIME で受け取れれば使い、無ければ拡張子 png とみなす。
        let mime = js_sys::Reflect::get(&entry, &JsValue::from_str("mime"))
            .ok()
            .and_then(|v| v.as_string());
        let image_type = match mime.as_deref() {
            Some(m) if !m.is_empty() => ImageType::MimeType(m),
            _ => ImageType::Extension("png"),
        };

        match Image::from_buffer(
            &bytes,
            image_type,
            CompressedImageFormats::NONE,
            true,
            ImageSampler::Default,
            RenderAssetUsages::default(),
        ) {
            Ok(image) => result.push(image),
            Err(err) => warn!("ブロック画像のデコードに失敗しました（要素 {i}）: {err}"),
        }
    }

    result
}

/// ネイティブビルドでは JS からの注入は無い（常に単色ブロックを使う）。
#[cfg(not(target_arch = "wasm32"))]
fn injected_brick_images() -> Vec<Image> {
    Vec::new()
}

/// ブロック用の画像（ハンドルと元のピクセル寸法）の一覧から、`index` 番目のブロックに
/// 割り当てる画像を返す。空なら `None`（＝単色ブロック）。個数を超えたら折り返して使う
/// （例: 2 枚なら偶数番=1 枚目・奇数番=2 枚目が交互に並ぶ）。
fn brick_image_for(images: &[(Handle<Image>, Vec2)], index: usize) -> Option<(Handle<Image>, Vec2)> {
    if images.is_empty() {
        None
    } else {
        Some(images[index % images.len()].clone())
    }
}

/// `content`（例: 画像のピクセル寸法）を `container`（例: アリーナ）に、アスペクト比を
/// 保ったまま内接させたときの表示寸法を返す（いわゆる "contain" フィット）。
/// 比率が合わない分は余白になる（呼び出し側で黒く塗る前提）。
fn contain_fit(content: Vec2, container: Vec2) -> Vec2 {
    let scale = (container.x / content.x).min(container.y / content.y);
    content * scale
}

/// 画像をアリーナに contain フィット（比率維持で内接・中央寄せ）で「そのまま」貼ったと仮定し、
/// `position` を中心・`size` を大きさとするブロックが覆う領域に対応する画像内の切り出し矩形
/// （ピクセル）を返す。ブロックが表示領域（内接矩形）からはみ出す場合は `None`（＝黒くする）。
/// 全ブロックが揃うと 1 枚の絵になり、ブロックを壊すとその穴から背後の背景画像が見える。
/// ワールド座標は y 上向き、画像座標は y 下向きなので v は上下反転して対応させる。
fn brick_image_rect(position: Vec2, size: Vec2, image_size: Vec2) -> Option<Rect> {
    let field = Vec2::new(RIGHT_WALL - LEFT_WALL, TOP_WALL - BOTTOM_WALL);
    // アリーナ中央に内接させた画像の表示寸法。中心原点なので範囲は [-half, half]。
    let display = contain_fit(image_size, field);
    let half = display / 2.0;

    let left = position.x - size.x / 2.0;
    let right = position.x + size.x / 2.0;
    let top = position.y + size.y / 2.0;
    let bottom = position.y - size.y / 2.0;

    // 内接矩形からはみ出すブロックには画像を貼らず、黒くする（余白＝黒）。
    if left < -half.x || right > half.x || bottom < -half.y || top > half.y {
        return None;
    }

    let u_min = (left + half.x) / display.x * image_size.x;
    let u_max = (right + half.x) / display.x * image_size.x;
    // 内接矩形の上端 (y=+half.y) を画像の上端 (v=0) に対応させる。
    let v_min = (half.y - top) / display.y * image_size.y;
    let v_max = (half.y - bottom) / display.y * image_size.y;

    Some(Rect::new(u_min, v_min, u_max, v_max))
}

/// 1 つのブロックを spawn する。`position` はワールド座標での中心、`size` はセルの大きさ。
/// `image` が `Some` なら、比率維持で貼った画像のうちこのブロックが覆う領域だけを切り出して
/// 表示する（引き伸ばしではなく「そのまま貼った絵の一部分」）。内接矩形の外や画像未指定なら
/// それぞれ黒・単色で描く。デフォルト配置と JS 注入配置の双方から使い、spawn ロジックを一本化する。
fn spawn_brick(
    commands: &mut Commands,
    position: Vec2,
    size: Vec2,
    image: Option<(Handle<Image>, Vec2)>,
) {
    // 画像・単色いずれも基準サイズを 1x1 にし、Transform.scale = セルサイズで拡大する
    // （壁・パドルと同じ方式）。画像スプライトは custom_size 未指定だと画像本来の
    // ピクセル寸法が基準になり、さらに scale が掛かって巨大化するため Vec2::ONE を明示する。
    // `rect` で画像内の切り出し領域を指定し、それを 1x1 の矩形に描く。
    let sprite = match image {
        Some((handle, image_size)) => match brick_image_rect(position, size, image_size) {
            Some(rect) => Sprite {
                image: handle,
                custom_size: Some(Vec2::ONE),
                rect: Some(rect),
                ..default()
            },
            // 内接矩形の外にあるブロックは黒（＝画像の余白と同じ扱い）。
            None => Sprite {
                color: Color::BLACK,
                ..default()
            },
        },
        None => Sprite {
            color: BRICK_COLOR,
            ..default()
        },
    };

    commands.spawn((
        sprite,
        Transform {
            translation: position.extend(0.0),
            scale: size.extend(1.0),
            ..default()
        },
        Brick,
        Collider,
    ));
}

fn main() {
    App::new()
        .insert_resource(BackgroundOverride(injected_background_image()))
        .insert_resource(BrickLayoutOverride(injected_brick_layout()))
        .insert_resource(BrickImagesOverride(injected_brick_images()))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                // Web ビルド時はこの ID の canvas 要素に描画する。
                // React 側の `<canvas id="bevy-canvas">` と一致させること。
                // ネイティブ実行時はこの指定は無視される。
                canvas: Some("#bevy-canvas".into()),
                // canvas を親要素のサイズにフィットさせる（React レイアウト側で制御可能に）。
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(Score(0))
        // 背景画像を比率維持で置くため、余白（レターボックス）は黒で塗る。
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Startup, setup)
        // Add our simulation systems to the update schedule
        // which is called once per frame.
        .add_systems(
            Update,
            (apply_velocity, move_paddle, check_for_collisions)
                // `chain`ing systems together runs them in order
                .chain(),
        )
        .add_systems(Update, update_scoreboard)
        .add_observer(play_collision_sound)
        .run();
}

#[derive(Component)]
struct Paddle;

#[derive(Component)]
struct Ball;

#[derive(Component, Deref, DerefMut)]
struct Velocity(Vec2);

#[derive(Event)]
struct BallCollided;

#[derive(Component)]
struct Brick;

#[derive(Resource, Deref)]
struct CollisionSound(Handle<AudioSource>);

// Default must be implemented to define this as a required component for the Wall component below
#[derive(Component, Default)]
struct Collider;

// This is a collection of the components that define a "Wall" in our game
#[derive(Component)]
#[require(Sprite, Transform, Collider)]
struct Wall;

/// Which side of the arena is this wall located on?
enum WallLocation {
    Left,
    Right,
    Bottom,
    Top,
}

impl WallLocation {
    /// Location of the *center* of the wall, used in `transform.translation()`
    fn position(&self) -> Vec2 {
        match self {
            WallLocation::Left => Vec2::new(LEFT_WALL, 0.),
            WallLocation::Right => Vec2::new(RIGHT_WALL, 0.),
            WallLocation::Bottom => Vec2::new(0., BOTTOM_WALL),
            WallLocation::Top => Vec2::new(0., TOP_WALL),
        }
    }

    /// (x, y) dimensions of the wall, used in `transform.scale()`
    fn size(&self) -> Vec2 {
        let arena_height = TOP_WALL - BOTTOM_WALL;
        let arena_width = RIGHT_WALL - LEFT_WALL;
        // Make sure we haven't messed up our constants
        assert!(arena_height > 0.0);
        assert!(arena_width > 0.0);

        match self {
            WallLocation::Left | WallLocation::Right => {
                Vec2::new(WALL_THICKNESS, arena_height + WALL_THICKNESS)
            }
            WallLocation::Bottom | WallLocation::Top => {
                Vec2::new(arena_width + WALL_THICKNESS, WALL_THICKNESS)
            }
        }
    }
}

impl Wall {
    // This "builder method" allows us to reuse logic across our wall entities,
    // making our code easier to read and less prone to bugs when we change the logic
    // Notice the use of Sprite and Transform alongside Wall, overwriting the default values defined for the required components
    fn new(location: WallLocation) -> (Wall, Sprite, Transform) {
        (
            Wall,
            Sprite::from_color(WALL_COLOR, Vec2::ONE),
            Transform {
                // We need to convert our Vec2 into a Vec3, by giving it a z-coordinate
                // This is used to determine the order of our sprites
                translation: location.position().extend(0.0),
                // The z-scale of 2D objects must always be 1.0,
                // or their ordering will be affected in surprising ways.
                // See https://github.com/bevyengine/bevy/issues/4149
                scale: location.size().extend(1.0),
                ..default()
            },
        )
    }
}

// This resource tracks the game's score
#[derive(Resource, Deref, DerefMut)]
struct Score(usize);

#[derive(Component)]
struct ScoreboardUi;

// Add the game's entities to our world
fn setup(
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

fn move_paddle(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut paddle_transform: Single<&mut Transform, With<Paddle>>,
    time: Res<Time>,
) {
    let mut direction = 0.0;

    if keyboard_input.pressed(KeyCode::ArrowLeft) {
        direction -= 1.0;
    }

    if keyboard_input.pressed(KeyCode::ArrowRight) {
        direction += 1.0;
    }

    // Calculate the new horizontal paddle position based on player input
    let new_paddle_position =
        paddle_transform.translation.x + direction * PADDLE_SPEED * time.delta_secs();

    // Update the paddle position,
    // making sure it doesn't cause the paddle to leave the arena
    let left_bound = LEFT_WALL + WALL_THICKNESS / 2.0 + PADDLE_SIZE.x / 2.0 + PADDLE_PADDING;
    let right_bound = RIGHT_WALL - WALL_THICKNESS / 2.0 - PADDLE_SIZE.x / 2.0 - PADDLE_PADDING;

    paddle_transform.translation.x = new_paddle_position.clamp(left_bound, right_bound);
}

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_secs();
        transform.translation.y += velocity.y * time.delta_secs();
    }
}

fn update_scoreboard(
    score: Res<Score>,
    score_root: Single<Entity, (With<ScoreboardUi>, With<Text>)>,
    mut writer: TextUiWriter,
) {
    *writer.text(*score_root, 1) = score.to_string();
}

fn check_for_collisions(
    mut commands: Commands,
    mut score: ResMut<Score>,
    ball_query: Single<(&mut Velocity, &Transform), With<Ball>>,
    collider_query: Query<(Entity, &Transform, Option<&Brick>), With<Collider>>,
) {
    let (mut ball_velocity, ball_transform) = ball_query.into_inner();

    for (collider_entity, collider_transform, maybe_brick) in &collider_query {
        let collision = ball_collision(
            BoundingCircle::new(ball_transform.translation.truncate(), BALL_DIAMETER / 2.),
            Aabb2d::new(
                collider_transform.translation.truncate(),
                collider_transform.scale.truncate() / 2.,
            ),
        );

        if let Some(collision) = collision {
            // Trigger observers of the "BallCollided" event
            commands.trigger(BallCollided);

            // Bricks should be despawned and increment the scoreboard on collision
            if maybe_brick.is_some() {
                commands.entity(collider_entity).despawn();
                **score += 1;
            }

            // Reflect the ball's velocity when it collides
            let mut reflect_x = false;
            let mut reflect_y = false;

            // Reflect only if the velocity is in the opposite direction of the collision
            // This prevents the ball from getting stuck inside the bar
            match collision {
                Collision::Left => reflect_x = ball_velocity.x > 0.0,
                Collision::Right => reflect_x = ball_velocity.x < 0.0,
                Collision::Top => reflect_y = ball_velocity.y < 0.0,
                Collision::Bottom => reflect_y = ball_velocity.y > 0.0,
            }

            // Reflect velocity on the x-axis if we hit something on the x-axis
            if reflect_x {
                ball_velocity.x = -ball_velocity.x;
            }

            // Reflect velocity on the y-axis if we hit something on the y-axis
            if reflect_y {
                ball_velocity.y = -ball_velocity.y;
            }
        }
    }
}

fn play_collision_sound(
    _collided: On<BallCollided>,
    mut commands: Commands,
    sound: Res<CollisionSound>,
) {
    commands.spawn((AudioPlayer(sound.clone()), PlaybackSettings::DESPAWN));
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum Collision {
    Left,
    Right,
    Top,
    Bottom,
}

// Returns `Some` if `ball` collides with `bounding_box`.
// The returned `Collision` is the side of `bounding_box` that `ball` hit.
fn ball_collision(ball: BoundingCircle, bounding_box: Aabb2d) -> Option<Collision> {
    if !ball.intersects(&bounding_box) {
        return None;
    }

    let closest = bounding_box.closest_point(ball.center());
    let offset = ball.center() - closest;
    let side = if offset.x.abs() > offset.y.abs() {
        if offset.x < 0. {
            Collision::Left
        } else {
            Collision::Right
        }
    } else if offset.y > 0. {
        Collision::Top
    } else {
        Collision::Bottom
    };

    Some(side)
}