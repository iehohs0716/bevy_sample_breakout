//! ゲームのエンティティを構成する Component / Resource / Event の定義。

use bevy::prelude::*;

use crate::config::{
    BOTTOM_WALL, LEFT_WALL, RIGHT_WALL, TOP_WALL, WALL_COLOR, WALL_THICKNESS,
};
use crate::injection::BrickLayout;

/// ゲーム全体の状態を Bevy の States で管理する。状態遷移は Rust 側が担い、
/// 状態に入った瞬間（`OnEnter`）に初期化やフロント(JS)への通知を行う（`notify` 参照）。
/// - `GameStart`: 開始待ち（初期状態）。進入時に盤面をリセットし、ボールはクリックまで静止。
///                左クリックで `Playing` へ遷移してボールが動き出す。
/// - `Playing`  : プレイ中。ゲームプレイ system はこの状態でのみ動く。
/// - `Cleared`  : 全ブロックを破壊した（クリア）。進入時に `breakout:gameclear` を通知。
/// - `GameOver` : ゲームオーバー（ライフ 0）。進入時、ネイティブは `GameStart` へ戻し、
///                WASM は `breakout:gameover` を通知して React に委ねる。
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    GameStart,
    Playing,
    Cleared,
    GameOver,
}

#[derive(Component)]
pub struct Paddle;

#[derive(Component)]
pub struct Ball;

#[derive(Component, Deref, DerefMut)]
pub struct Velocity(pub Vec2);

#[derive(Event)]
pub struct BallCollided;

#[derive(Component)]
pub struct Brick;

#[derive(Resource, Deref)]
pub struct CollisionSound(pub Handle<AudioSource>);

// Default must be implemented to define this as a required component for the Wall component below
#[derive(Component, Default)]
pub struct Collider;

// This is a collection of the components that define a "Wall" in our game
#[derive(Component)]
#[require(Sprite, Transform, Collider)]
pub struct Wall;

/// Which side of the arena is this wall located on?
/// 下端は反射する壁ではなく `DeathZone`（ボールが触れるとライフが減る領域）にしたため、
/// `Bottom` は持たない。
pub enum WallLocation {
    Left,
    Right,
    Top,
}

impl WallLocation {
    /// Location of the *center* of the wall, used in `transform.translation()`
    fn position(&self) -> Vec2 {
        match self {
            WallLocation::Left => Vec2::new(LEFT_WALL, 0.),
            WallLocation::Right => Vec2::new(RIGHT_WALL, 0.),
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
            WallLocation::Top => Vec2::new(arena_width + WALL_THICKNESS, WALL_THICKNESS),
        }
    }
}

impl Wall {
    // This "builder method" allows us to reuse logic across our wall entities,
    // making our code easier to read and less prone to bugs when we change the logic
    // Notice the use of Sprite and Transform alongside Wall, overwriting the default values defined for the required components
    pub fn new(location: WallLocation) -> (Wall, Sprite, Transform) {
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

/// アリーナ下端の「死亡ゾーン」。反射する `Wall` とは違い、ボールが触れると
/// ライフを減らす（0 になれば `GameOver`）。見た目は持たず、衝突判定用の矩形領域
/// （`Transform` の scale が大きさ）としてのみ存在する。`Collider` を持つので
/// `check_for_collisions` の衝突判定対象になる。
#[derive(Component)]
pub struct DeathZone;

impl DeathZone {
    /// 旧 `WallLocation::Bottom` の位置・大きさをそのまま引き継ぐ（下端に横一列）。
    pub fn new() -> (DeathZone, Transform, Collider) {
        let arena_width = RIGHT_WALL - LEFT_WALL;
        (
            DeathZone,
            Transform {
                translation: Vec2::new(0., BOTTOM_WALL).extend(0.0),
                scale: Vec2::new(arena_width + WALL_THICKNESS, WALL_THICKNESS).extend(1.0),
                ..default()
            },
            Collider,
        )
    }
}

// This resource tracks the game's score
#[derive(Resource, Deref, DerefMut)]
pub struct Score(pub usize);

#[derive(Component)]
pub struct ScoreboardUi;

// 残りライフを保持する Resource。ボールが DeathZone に触れるたびに 1 減る。
#[derive(Resource, Deref, DerefMut)]
pub struct Lives(pub usize);

#[derive(Component)]
pub struct LivesUi;

/// 再スタート（`OnEnter(GameStart)`）でブロックを再配置するため、確定済みの配置と画像を
/// 保持する Resource。`setup` で 1 度だけ確定させ、`reset_game` が読んで spawn する。
/// これにより、消費済みの JS 注入パラメータに再アクセスせずとも盤面を作り直せる。
#[derive(Resource)]
pub struct GameAssets {
    pub brick_layout: BrickLayout,
    pub brick_image: Option<(Handle<Image>, Vec2)>,
}
