use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::components::{Brick, BrickCell, BrickFill, BrokenEdges, Collider};
use crate::config::BRICK_COLOR;
use crate::common::brick::mesh::{build_brick_mesh, build_brick_material};
use crate::common::brick::texture_crop::brick_uv_rect;

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
