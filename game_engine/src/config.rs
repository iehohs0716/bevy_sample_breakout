//! ゲーム全体で使う定数（アリーナ寸法・各要素のサイズ・色・アセットパス）。

use bevy::prelude::*;

// These constants are defined in `Transform` units.
// Using the default 2D camera they correspond 1:1 with screen pixels.
pub const PADDLE_SIZE: Vec2 = Vec2::new(120.0, 20.0);
pub const GAP_BETWEEN_PADDLE_AND_FLOOR: f32 = 60.0;
pub const PADDLE_SPEED: f32 = 500.0;
// How close can the paddle get to the wall
pub const PADDLE_PADDING: f32 = 10.0;

// We set the z-value of the ball to 1 so it renders on top in the case of overlapping sprites.
pub const BALL_STARTING_POSITION: Vec3 = Vec3::new(0.0, -200.0, 1.0);
pub const BALL_DIAMETER: f32 = 15.;
pub const BALL_SPEED: f32 = 400.0;
pub const INITIAL_BALL_DIRECTION: Vec2 = Vec2::new(0.5, -0.5);

pub const WALL_THICKNESS: f32 = 10.0;
// x coordinates
pub const LEFT_WALL: f32 = -450.;
pub const RIGHT_WALL: f32 = 450.;
// y coordinates
pub const BOTTOM_WALL: f32 = -300.;
pub const TOP_WALL: f32 = 300.;

// プレイヤーの初期ライフ数。ボールが下端（DeathZone）に触れるたびに 1 減り、
// 0 になると GameOver へ遷移する。
pub const INITIAL_LIVES: usize = 5;

pub const BRICK_SIZE: Vec2 = Vec2::new(50., 30.);
// These values are exact
pub const GAP_BETWEEN_PADDLE_AND_BRICKS: f32 = 270.0;
pub const GAP_BETWEEN_BRICKS: f32 = 0.0;
// These values are lower bounds, as the number of bricks is computed
pub const GAP_BETWEEN_BRICKS_AND_CEILING: f32 = 0.0;
pub const GAP_BETWEEN_BRICKS_AND_SIDES: f32 = 0.0;

pub const SCOREBOARD_FONT_SIZE: FontSize = FontSize::Px(33.0);
pub const SCOREBOARD_TEXT_PADDING: Val = Val::Px(5.0);

// 背景画像のデフォルトパス（AssetServer 基準。Web では `/assets/` 配下、ネイティブでは
// `game_engine/assets/` 配下を指す）。
//
// Web ビルドでは、React 側が `window.__BREAKOUT_CONFIG__.backgroundBytes` に
// fetch 済みの画像バイト列（Uint8Array）を載せていれば、それを優先して背景に使う。
// これにより「アプリのコードは1つ」のまま、サービスごとに（S3 等の任意 URL の画像でも）
// 背景だけを差し替えられる。React 側が bytes を渡さない場合はこのデフォルトにフォールバックする。
pub const BACKGROUND_IMAGE_PATH: &str = "backgrounds/background.png";
// 背景スプライトの表示サイズ（アリーナ全体を覆う）。
pub const BACKGROUND_SIZE: Vec2 = Vec2::new(RIGHT_WALL - LEFT_WALL, TOP_WALL - BOTTOM_WALL);

pub const PADDLE_COLOR: Color = Color::srgb(0.3, 0.3, 0.7);
pub const BALL_COLOR: Color = Color::srgb(1.0, 0.5, 0.5);
pub const BRICK_COLOR: Color = Color::srgb(0.5, 0.5, 1.0);
// 無傷（未破壊）のブロック同士は見た目が繋がって1枚の絵に見えるため、個々のブロックの境界が
// うっすら見えるよう、低アルファの輪郭線を重ねて描く（`systems::update::brick::draw_brick_outlines`）。
pub const BRICK_OUTLINE_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.25);
pub const WALL_COLOR: Color = Color::srgb(0.8, 0.8, 0.8);
pub const TEXT_COLOR: Color = Color::srgb(0.5, 0.5, 1.0);
pub const SCORE_COLOR: Color = Color::srgb(1.0, 0.5, 0.5);

// 画像差分による自動ブロック生成（`injection::diff_brick_layout`）関連の定数。
// 背景画像とブロック画像の両方が注入され、かつブロック配置が明示指定されていない場合にのみ使う。
/// 自動生成を許可する高さの下限比率。0.0 = バー（パドル）の位置、1.0 = アリーナ天井
/// （`TOP_WALL`）。この比率未満の高さには（画像差分があっても）ブロックを生成しない。
pub const BRICK_DIFF_LAYOUT_MIN_HEIGHT_RATIO: f32 = 0.2;
/// 背景画像とブロック画像の同一セル領域の平均色を比較し、ブロックを生成するかどうかを判定する
/// しきい値（RGB のユークリッド距離。各チャンネル 0..1 スケールなので最大 `sqrt(3)` 程度）。
/// これを超えたセルだけを「差分あり」としてブロックにする。実画像で試して調整する前提の仮値。
pub const BRICK_DIFF_COLOR_THRESHOLD: f32 = 0.1;

// ブロック破壊面のギザギザ（中点変位法）関連の定数。
/// 中点変位の再帰段数。段数ぶん辺が細分化される。
pub const TEAR_DEPTH: u32 = 2;
/// 振れ幅の辺長に対する比率。既定のブロック寸法(50x30)の半分より十分小さく、
/// 中心からの扇形三角形分割が破綻しない(star-shaped from center)範囲に収まる値。
pub const TEAR_ROUGHNESS: f32 = 0.45;
