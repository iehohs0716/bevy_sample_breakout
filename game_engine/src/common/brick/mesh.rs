//! ブロックのメッシュ・マテリアル構築。破壊された辺だけを中点変位法のギザギザ輪郭に
//! 再構築する処理（兄弟モジュール `torn_edge` を使う）を含む、ブロック描画のうち幾何処理だけを持つ。
//! `spawn_brick` / `redraw_broken_bricks`（親モジュール）からのみ使うため非公開のまま。

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use crate::components::{BrickCell, BrickFill, BrokenEdges};

pub(super) fn build_brick_material(fill: &BrickFill) -> ColorMaterial {
    match fill {
        BrickFill::Color(color) => ColorMaterial::from(*color),
        // `image` は `&Handle<Image>`。`image.clone()` は `&T` に常に生える `Clone`（参照自体の
        // コピー）に解決されて `&Handle<Image>` を返してしまう（E0308）ため、`Handle::clone` を
        // 明示して `Handle<Image>` 本体を複製する。
        BrickFill::Textured { image, .. } => ColorMaterial {
            texture: Some(Handle::clone(image)),
            ..default()
        },
    }
}

/// ローカル座標 `p`（ブロック中心原点、範囲おおよそ [-size/2, size/2]）を、`fill` が
/// `Textured` の場合の UV に変換する。`Color` の場合は使われないので `[0.0, 0.0]` を返す。
/// ワールド座標は y 上向き、画像座標は y 下向きなので v は反転する。ギザギザの変位で p が
/// わずかに範囲外へ出ても、同じ1枚絵の続きをサンプリングするだけなので clamp しない。
fn vertex_uv(p: Vec2, size: Vec2, fill: &BrickFill) -> [f32; 2] {
    match fill {
        BrickFill::Color(_) => [0.0, 0.0],
        BrickFill::Textured { uv_rect, .. } => {
            let t = Vec2::new(p.x / size.x + 0.5, 0.5 - p.y / size.y);
            let uv = uv_rect.min + t * uv_rect.size();
            [uv.x, uv.y]
        }
    }
}

/// ローカル原点(0,0)中心、四隅 `(±size.x/2, ±size.y/2)` の矩形を輪郭とし、`broken` で
/// true になっている辺だけ中点変位法のギザギザに置き換えたメッシュを構築する。中心
/// (`Vec2::ZERO`)を追加した扇形三角形分割（fan triangulation）を使うため、輪郭は常に
/// 中心から見える(star-shaped)範囲に収める必要がある（`TEAR_ROUGHNESS` 参照）。
pub(crate) fn build_brick_mesh(
    size: Vec2,
    cell: BrickCell,
    broken: &BrokenEdges,
    fill: &BrickFill,
) -> Mesh {
    let half = size / 2.0;
    let corners = [
        Vec2::new(-half.x, -half.y), // bottom-left
        Vec2::new(half.x, -half.y),  // bottom-right
        Vec2::new(half.x, half.y),   // top-right
        Vec2::new(-half.x, half.y),  // top-left
    ];
    // 下→右→上→左の順（反時計回り）。edge_index は 下=0, 右=1, 上=2, 左=3 で固定する。
    let edge_broken = [broken.bottom, broken.right, broken.top, broken.left];

    let mut boundary: Vec<Vec2> = Vec::new();
    for i in 0..4 {
        let start = corners[i];
        let end = corners[(i + 1) % 4];
        boundary.push(start);
        if edge_broken[i] {
            // 破れた辺だけ、中点変位法のギザギザ輪郭に置き換える（実装は `torn_edge` モジュール）。
            // この中で、boundaryに追加の頂点が追加されていく
            super::torn_edge::push_torn_edge(cell, i as u32, start, end, &mut boundary);
        }
    }

    let n = boundary.len();
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n + 1);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(n + 1);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(n + 1);

    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, 0.0, 1.0]);
    uvs.push(vertex_uv(Vec2::ZERO, size, fill));

    for p in &boundary {
        positions.push([p.x, p.y, 0.0]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push(vertex_uv(*p, size, fill));
    }

    let mut indices: Vec<u32> = Vec::with_capacity(n * 3);
    for i in 0..n {
        let a = (i + 1) as u32;
        let b = (((i + 1) % n) + 1) as u32;
        indices.push(0);
        indices.push(a);
        indices.push(b);
    }

    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BRICK_COLOR, BRICK_SIZE};

    fn all_broken() -> BrokenEdges {
        BrokenEdges { top: true, bottom: true, left: true, right: true }
    }

    /// 中心(`Vec2::ZERO`)からの扇形三角形分割が破綻しない(star-shaped)ことを固定化する
    /// 回帰テスト。境界点を中心から見た角度が、輪郭を1周する間に単調に増加していれば
    /// (=同じ角度を2度跨がず一方向に回る)star-shaped である。全辺破壊(最も変位点が多い
    /// 最悪ケース)を、cell を変えて何通りも検証する。
    #[test]
    fn build_brick_mesh_is_star_shaped_for_many_cells() {
        let fill = BrickFill::Color(BRICK_COLOR);
        for row in 0..30 {
            for col in 0..30 {
                let cell = BrickCell { row, col };
                let mesh = build_brick_mesh(BRICK_SIZE, cell, &all_broken(), &fill);
                assert_star_shaped(&mesh, row, col);
            }
        }
    }

    /// 一部の辺だけ破壊されたケース(全16通りの辺組み合わせ)でも star-shaped が崩れないことを確認する。
    #[test]
    fn build_brick_mesh_is_star_shaped_for_partial_breaks() {
        let fill = BrickFill::Color(BRICK_COLOR);
        let cell = BrickCell { row: 3, col: 7 };
        for mask in 0..16u8 {
            let broken = BrokenEdges {
                top: mask & 1 != 0,
                bottom: mask & 2 != 0,
                left: mask & 4 != 0,
                right: mask & 8 != 0,
            };
            let mesh = build_brick_mesh(BRICK_SIZE, cell, &broken, &fill);
            assert_star_shaped(&mesh, mask as i32, 0);
        }
    }

    /// メッシュの頂点(先頭は中心、以降が境界)を取り出し、中心から見た角度が輪郭を1周する間
    /// 単調増加であることを検証する（star-shaped の判定）。
    fn assert_star_shaped(mesh: &Mesh, row: i32, col: i32) {
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .expect("position attribute must exist");
        let bevy::render::mesh::VertexAttributeValues::Float32x3(positions) = positions else {
            panic!("unexpected position attribute format");
        };

        // 先頭(index 0)は中心。以降が境界点。
        let boundary: Vec<Vec2> = positions[1..].iter().map(|p| Vec2::new(p[0], p[1])).collect();
        assert!(
            boundary.len() >= 4,
            "row={row} col={col}: 境界点が矩形の4頂点未満"
        );

        let angles: Vec<f32> = boundary.iter().map(|p| p.y.atan2(p.x)).collect();

        // 角度を [0, 2π) に正規化した上で、隣接点間の増分が常に正（かつ 2π 未満）であることを
        // 確認する。これが崩れる = 角度が後退/飛び越える = 中心から見えない箇所がある =
        // star-shaped ではない = 扇形三角形分割が破綻する。
        let two_pi = std::f32::consts::TAU;
        let normalized: Vec<f32> = angles.iter().map(|a| a.rem_euclid(two_pi)).collect();
        let n = normalized.len();
        for i in 0..n {
            let a = normalized[i];
            let b = normalized[(i + 1) % n];
            let delta = (b - a).rem_euclid(two_pi);
            assert!(
                delta > 0.0 && delta < two_pi,
                "row={row} col={col}: 境界点{i}→{}の角度が単調増加でない(star-shapedが崩れている)",
                (i + 1) % n
            );
        }
    }
}
