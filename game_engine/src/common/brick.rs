//! ブロックの spawn / 再構築ヘルパー。ドメイン型に一切依存しない純粋な座標計算
//! （`contain_fit`）だけは `util` に置き、それ以外のブロック固有の処理はここにまとめる。
//! `systems::setup` と `systems::reset_game` の両方から呼ばれるが、中身は全てブロックという
//! 単一ドメインの処理であり、複数ドメインの共通置き場ではない。
//! メッシュ・マテリアル構築は子モジュール `mesh` に、テクスチャ切り出し計算は子モジュール
//! `texture_crop` に、破れた辺のギザギザ輪郭生成は子モジュール `torn_edge` に分離し、
//! ここには spawn という ECS 向けの入り口だけを残す。`Query` を取る実際の Bevy system
//! （`redraw_broken_bricks` 等）は `systems::update::brick` 側に置く（`build_brick_mesh` を
//! `crate::common::build_brick_mesh` として公開し、そこから呼べるようにしている）。
//!
//! 画像は「引き伸ばし（テクスチャ漬け）」ではなく、盤面に比率維持で貼った 1 枚の絵として扱い、
//! 各ブロックはその絵のうち自分が覆う領域だけを切り出して表示する。全ブロックが揃うと 1 枚の
//! 絵になり、ブロックを壊すとその穴から背後の背景画像が見える。
//!
//! ブロックは Sprite ではなく Mesh2d(動的メッシュ) + ColorMaterial で描画する。壁・パドルと違い
//! ブロックは破壊された隣との接触面だけを中点変位法のギザギザ輪郭に再構築する必要があり、
//! それには頂点を自前で持てるメッシュが要る。

mod mesh;
use mesh::build_brick_material;
pub(crate) use mesh::build_brick_mesh;

mod texture_crop;
use texture_crop::brick_uv_rect;

mod torn_edge;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::components::{Brick, BrickCell, BrickFill, BrokenEdges, Collider};
use crate::config::BRICK_COLOR;

/// ブロックの spawn / 再構築に要る 2 つの `Assets` をまとめた `SystemParam`。
/// `spawn_brick` を呼ぶ `setup`・`reset_game` の両方でこの 2 つは常にセットで必要になるため、
/// バラのまま渡すと（他の必須パラメータと合わせて）`clippy::too_many_arguments` を誘発する。
/// 意味的にも「ブロック描画に使うアセット」という 1 つのまとまりなので、
/// 型として束ねてしまう方が呼び出し側の引数リストも意図も単純になる。
#[derive(SystemParam)]
pub struct BrickAssets<'w> {
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub materials: ResMut<'w, Assets<ColorMaterial>>,
}

/// 1 つのブロックを spawn する。`position` はワールド座標での中心、`size` はセルの大きさ、
/// `cell` は盤面上の行・列（隣接判定・ギザギザの種の両方に使う）。
/// `image` が `Some` なら、比率維持で貼った画像のうちこのブロックが覆う領域だけを切り出して
/// 表示する（引き伸ばしではなく「そのまま貼った絵の一部分」）。内接矩形の外や画像未指定なら
/// それぞれ黒・単色で描く。デフォルト配置と JS 注入配置の双方から使い、spawn ロジックを一本化する。
pub fn spawn_brick(
    commands: &mut Commands,
    brick_assets: &mut BrickAssets,
    position: Vec2,
    size: Vec2,
    cell: BrickCell,
    image: Option<(Handle<Image>, Vec2)>,
) {
    let fill = match image {
        Some((handle, image_size)) => match brick_uv_rect(position, size, image_size) {
            Some(uv_rect) => BrickFill::Textured { image: handle, uv_rect },
            // 内接矩形の外にあるブロックは黒（＝画像の余白と同じ扱い）。
            None => BrickFill::Color(Color::BLACK),
        },
        None => BrickFill::Color(BRICK_COLOR),
    };

    let broken = BrokenEdges::default();
    let mesh = build_brick_mesh(size, cell, &broken, &fill);
    let material = build_brick_material(&fill);

    commands.spawn((
        Mesh2d(brick_assets.meshes.add(mesh)),
        MeshMaterial2d(brick_assets.materials.add(material)),
        Transform::from_translation(position.extend(0.0)),
        // 不変データ(大きさ・格子座標・塗り方)は Brick にまとめて持たせる。
        Brick { size, cell, fill },
        Collider,
        // 実行中に変化する破れ状態だけ独立コンポーネント。
        broken,
    ));
}
