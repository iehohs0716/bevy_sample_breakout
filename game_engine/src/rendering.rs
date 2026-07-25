//! 画像のフィット計算とブロックの描画（spawn / メッシュ構築）ヘルパー。
//!
//! 画像は「引き伸ばし（テクスチャ漬け）」ではなく、盤面に比率維持で貼った 1 枚の絵として扱い、
//! 各ブロックはその絵のうち自分が覆う領域だけを切り出して表示する。全ブロックが揃うと 1 枚の
//! 絵になり、ブロックを壊すとその穴から背後の背景画像が見える。
//!
//! ブロックは Sprite ではなく Mesh2d(動的メッシュ) + ColorMaterial で描画する。壁・パドルと違い
//! ブロックは破壊された隣との接触面だけを中点変位法のギザギザ輪郭に再構築する必要があり、
//! それには頂点を自前で持てるメッシュが要る。

use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use crate::components::{Brick, BrickCell, BrickFill, BrokenEdges, Collider};
use crate::config::{BOTTOM_WALL, BRICK_COLOR, LEFT_WALL, RIGHT_WALL, TOP_WALL};

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

/// `content`（例: 画像のピクセル寸法）を `container`（例: アリーナ）に、アスペクト比を
/// 保ったまま内接させたときの表示寸法を返す（いわゆる "contain" フィット）。
/// 比率が合わない分は余白になる（呼び出し側で黒く塗る前提）。
pub fn contain_fit(content: Vec2, container: Vec2) -> Vec2 {
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

/// ピクセル矩形を `image_size` で割り、0..1 の UV 矩形に正規化する。
fn normalize_rect(rect: Rect, image_size: Vec2) -> Rect {
    Rect::new(
        rect.min.x / image_size.x,
        rect.min.y / image_size.y,
        rect.max.x / image_size.x,
        rect.max.y / image_size.y,
    )
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
        Some((handle, image_size)) => match brick_image_rect(position, size, image_size) {
            Some(rect) => BrickFill::Textured {
                image: handle,
                uv_rect: normalize_rect(rect, image_size),
            },
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

/// `BrokenEdges` が変化したブロックだけ、メッシュを壊れた輪郭で再構築する。
pub fn redraw_broken_bricks(
    mut meshes: ResMut<Assets<Mesh>>,
    bricks: Query<(&Brick, &BrokenEdges, &Mesh2d), Changed<BrokenEdges>>,
) {
    for (brick, broken_edge, mesh2d) in &bricks {
        if let Some(mut mesh) = meshes.get_mut(&mesh2d.0) {
            *mesh = build_brick_mesh(brick.size, brick.cell, broken_edge, &brick.fill);
        }
    }
}

fn build_brick_material(fill: &BrickFill) -> ColorMaterial {
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
fn build_brick_mesh(size: Vec2, cell: BrickCell, broken: &BrokenEdges, fill: &BrickFill) -> Mesh {
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
            // 破れた辺だけ、中点変位法のギザギザ輪郭に置き換える（実装は `tear` モジュール）。
            // この中で、boundaryに追加の頂点が追加されていく
            crate::tear::push_torn_edge(cell, i as u32, start, end, &mut boundary);
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
    use crate::config::BRICK_SIZE;

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
