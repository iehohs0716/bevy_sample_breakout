//! ブロック関連のヘルパーへの入り口。詳細は子モジュール `brick` を参照。
//! `build_brick_mesh` は `systems::update::brick::redraw_broken_bricks`（実際の Bevy system）だけが
//! 追加で必要とするため、`pub(crate)` でクレート内にだけ公開している。

mod brick;
pub use brick::{spawn_brick, BrickAssets};
pub(crate) use brick::build_brick_mesh;
