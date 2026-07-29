//! アリーナ（プレイ領域）の境界ジオメトリと、そこに配置される `Wall` / `DeathZone` の定義。

use bevy::prelude::*;

use crate::config::{BOTTOM_WALL, LEFT_WALL, RIGHT_WALL, TOP_WALL, WALL_COLOR, WALL_THICKNESS};

// Default must be implemented to define this as a required component for the Wall component below
#[derive(Component, Default)]
pub struct Collider;

// This is a collection of the components that define a "Wall" in our game
#[derive(Component)]
#[require(Sprite, Transform, Collider)]
pub struct Wall;

/// アリーナ（プレイ領域）を囲む 4 辺の**ゲーム境界ジオメトリ**。`Wall::new` / `DeathZone::new`
/// からのみ使う実装詳細なので、モジュール外へは公開しない。
enum ArenaEdge {
    Left,
    Right,
    Top,
    Bottom,
}

impl ArenaEdge {
    /// 辺の中心のワールド座標（`Transform.translation` 用）。
    pub fn position(&self) -> Vec2 {
        match self {
            ArenaEdge::Left => Vec2::new(LEFT_WALL, 0.),
            ArenaEdge::Right => Vec2::new(RIGHT_WALL, 0.),
            ArenaEdge::Top => Vec2::new(0., TOP_WALL),
            ArenaEdge::Bottom => Vec2::new(0., BOTTOM_WALL),
        }
    }

    /// 辺の大きさ（`Transform.scale` 用）。左右は縦長・上下は横長で、いずれも壁厚ぶん端を
    /// 伸ばして角の隙間を塞ぐ。
    pub fn size(&self) -> Vec2 {
        let arena_height = TOP_WALL - BOTTOM_WALL;
        let arena_width = RIGHT_WALL - LEFT_WALL;
        // Make sure we haven't messed up our constants
        assert!(arena_height > 0.0);
        assert!(arena_width > 0.0);

        match self {
            ArenaEdge::Left | ArenaEdge::Right => {
                Vec2::new(WALL_THICKNESS, arena_height + WALL_THICKNESS)
            }
            ArenaEdge::Top | ArenaEdge::Bottom => {
                Vec2::new(arena_width + WALL_THICKNESS, WALL_THICKNESS)
            }
        }
    }
}

/// 壁を作れるアリーナの辺。`Bottom` を**あえて持たない**ことで「下端の反射壁」を型レベルで
/// 表現不能にする番人（下端は `DeathZone`）。列挙子が Left/Right/Top の 3 つなのは重複ではなく
/// 「壁は 3 辺にしか作れない」という制約そのもの。幾何は持たず、`ArenaEdge` へ `From` で写して
/// 委譲する（幾何の定義は `ArenaEdge` 1 箇所に集約）。
pub enum WallLocation {
    Left,
    Right,
    Top,
}

/// 壁の辺 → アリーナ辺（幾何）への変換。`WallLocation` は 3 辺しか無いので、この変換から
/// `ArenaEdge::Bottom` は決して生まれない。
impl From<WallLocation> for ArenaEdge {
    fn from(location: WallLocation) -> Self {
        match location {
            WallLocation::Left => ArenaEdge::Left,
            WallLocation::Right => ArenaEdge::Right,
            WallLocation::Top => ArenaEdge::Top,
        }
    }
}

impl Wall {
    // This "builder method" allows us to reuse logic across our wall entities,
    // making our code easier to read and less prone to bugs when we change the logic
    // Notice the use of Sprite and Transform alongside Wall, overwriting the default values defined for the required components
    pub fn new(location: WallLocation) -> (Wall, Sprite, Transform) {
        // 幾何は ArenaEdge に一元化。反射壁になり得る 3 辺だけが渡ってくる。
        let edge: ArenaEdge = location.into();
        (
            Wall,
            Sprite::from_color(WALL_COLOR, Vec2::ONE),
            Transform {
                // We need to convert our Vec2 into a Vec3, by giving it a z-coordinate
                // This is used to determine the order of our sprites
                translation: edge.position().extend(0.0),
                // The z-scale of 2D objects must always be 1.0,
                // or their ordering will be affected in surprising ways.
                // See https://github.com/bevyengine/bevy/issues/4149
                scale: edge.size().extend(1.0),
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
    /// アリーナ下端（`ArenaEdge::Bottom`）の位置・大きさをそのまま使う（下端に横一列）。
    /// 幾何は `Wall` と同じ `ArenaEdge` 由来だが、反射ではなくライフ減の役割は DeathZone 側が担う。
    pub fn new() -> (DeathZone, Transform, Collider) {
        (
            DeathZone,
            Transform {
                translation: ArenaEdge::Bottom.position().extend(0.0),
                scale: ArenaEdge::Bottom.size().extend(1.0),
                ..default()
            },
            Collider,
        )
    }
}
