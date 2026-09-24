//! ブロック（レンガ）に関連するドメイン処理のモジュール群。
//!
//! - `layout`: ブロックの配置計算（画像差分、デフォルト配置など）
//! - `mesh` / `torn_edge`: ギザギザ輪郭を含む動的メッシュ(Mesh2d)の生成
//! - `texture_crop`: 背景画像から自身が覆う領域を切り出すUV計算
//! - `spawn`: ECSへのエンティティ生成インターフェース

mod mesh;
pub(crate) use mesh::build_brick_mesh;

mod texture_crop;

pub mod layout;
pub mod spawn;
pub use spawn::{spawn_brick, BrickAssets};

