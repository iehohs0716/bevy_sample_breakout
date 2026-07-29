//! ゲームのエンティティを構成する Component / Resource / Event の定義。
//!
//! 状態遷移は子モジュール `state`、ブロック関連は `brick`、アリーナ境界（壁・デスゾーン）は
//! `arena`、スコア・ライフの UI は `scoreboard` にそれぞれ分離し、ここで `pub use` して外
//! （`systems` / `setup` / `common` など）からは `components::` 経由でまとめて見えるように
//! している。パドル・ボール・衝突音・アセットなど、他ドメインと薄く結合した小さな要素は
//! 分離せずこのファイル直下に残す。

mod state;
pub use state::GameState;

mod brick;
pub use brick::{Brick, BrickCell, BrickDestroyed, BrickFill, BrokenEdges};

mod arena;
pub use arena::{Collider, DeathZone, Wall, WallLocation};

mod scoreboard;
pub use scoreboard::{Lives, LivesUi, Score, ScoreboardUi};

use bevy::prelude::*;

use crate::injection::BrickLayout;

#[derive(Component)]
pub struct Paddle;

#[derive(Component)]
pub struct Ball;

#[derive(Component, Deref, DerefMut)]
pub struct Velocity(pub Vec2);

#[derive(Event)]
pub struct BallCollided;

#[derive(Resource, Deref)]
pub struct CollisionSound(pub Handle<AudioSource>);

/// 再スタート（`OnEnter(GameStart)`）でブロックを再配置するため、確定済みの配置と画像を
/// 保持する Resource。`setup` で 1 度だけ確定させ、`reset_game` が読んで spawn する。
/// これにより、消費済みの JS 注入パラメータに再アクセスせずとも盤面を作り直せる。
#[derive(Resource)]
pub struct GameAssets {
    pub brick_layout: BrickLayout,
    pub brick_image: Option<(Handle<Image>, Vec2)>,
}
