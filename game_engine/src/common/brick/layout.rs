//! アリーナを敷き詰めるデフォルト配置や、画像差分からの自動配置など、
//! ドメインロジックとしてのブロック配置決定処理。

use bevy::prelude::*;

use crate::components::BrickCell;
use crate::config::{
    BOTTOM_WALL, BRICK_DIFF_COLOR_THRESHOLD, BRICK_DIFF_LAYOUT_MIN_HEIGHT_RATIO,
    GAP_BETWEEN_BRICKS, GAP_BETWEEN_BRICKS_AND_CEILING, GAP_BETWEEN_BRICKS_AND_SIDES,
    GAP_BETWEEN_PADDLE_AND_BRICKS, LEFT_WALL, RIGHT_WALL, TOP_WALL,
};
use crate::util::{average_color, inscribed_source_rect};

// ブロック配置。座標は Bevy ワールド座標
// （中心原点・y 上向き・1 単位 = 1px。アリーナは x∈[LEFT_WALL, RIGHT_WALL],
// y∈[BOTTOM_WALL, TOP_WALL]）で、各ブロックの *中心* 位置を表す。
// `cell_size` は全ブロック共通のセルの大きさ（幅・高さ）。
pub struct BrickLayout {
    pub positions: Vec<Vec2>,
    pub cell_size: Vec2,
    pub cells: Vec<BrickCell>,
}

/// アリーナを敷き詰めるブロックグリッドの全候補セル（`position`, `BrickCell`）を計算する。
/// `default_brick_layout`（無条件に全セルを採用）と `diff_brick_layout`（一部だけを間引く）の
/// 両方が同じグリッド形状を前提にするための共有ロジック。`paddle_y` はパドルの中心 y で、
/// ブロック帯はその上に `GAP_BETWEEN_PADDLE_AND_BRICKS` だけ空けて始まる。`cell_size` は
/// 1 セルの大きさ（JS が `cellSize` を指定していればそれ、無ければ `BRICK_SIZE`。呼び出し側が
/// 解決する）。
fn brick_grid_candidates(paddle_y: f32, cell_size: Vec2) -> (Vec<Vec2>, Vec<BrickCell>) {
    let total_width_of_bricks = (RIGHT_WALL - LEFT_WALL) - 2. * GAP_BETWEEN_BRICKS_AND_SIDES;
    let bottom_edge_of_bricks = paddle_y + GAP_BETWEEN_PADDLE_AND_BRICKS;
    let total_height_of_bricks = TOP_WALL - bottom_edge_of_bricks - GAP_BETWEEN_BRICKS_AND_CEILING;

    assert!(total_width_of_bricks > 0.0);
    assert!(total_height_of_bricks > 0.0);

    // 使える面積に何行・何列入るか（切り捨て）。
    let n_columns = (total_width_of_bricks / (cell_size.x + GAP_BETWEEN_BRICKS)).floor() as usize;
    let n_rows = (total_height_of_bricks / (cell_size.y + GAP_BETWEEN_BRICKS)).floor() as usize;
    let n_vertical_gaps = n_columns - 1;

    // 列数を丸めたぶん帯の幅は領域より狭くなるので、中心から帯の半分だけ戻して中央揃えにする。
    let center_of_bricks = (LEFT_WALL + RIGHT_WALL) / 2.0;
    let left_edge_of_bricks = center_of_bricks
        - (n_columns as f32 / 2.0 * cell_size.x)
        - n_vertical_gaps as f32 / 2.0 * GAP_BETWEEN_BRICKS;

    // `translation` は中心座標なので、左下の縁から半セルぶん内側を最初のブロックの中心にする。
    let offset_x = left_edge_of_bricks + cell_size.x / 2.;
    let offset_y = bottom_edge_of_bricks + cell_size.y / 2.;

    let mut positions = Vec::with_capacity(n_rows * n_columns);
    let mut cells = Vec::with_capacity(n_rows * n_columns);
    for row in 0..n_rows {
        for column in 0..n_columns {
            positions.push(Vec2::new(
                offset_x + column as f32 * (cell_size.x + GAP_BETWEEN_BRICKS),
                offset_y + row as f32 * (cell_size.y + GAP_BETWEEN_BRICKS),
            ));
            cells.push(BrickCell {
                row: row as i32,
                col: column as i32,
            });
        }
    }

    (positions, cells)
}

/// アリーナを敷き詰めるデフォルトのブロック配置を計算して返す。
/// `cell_size` は呼び出し側（`setup`）が `injected_cell_size().unwrap_or(BRICK_SIZE)`
/// などで解決したものを渡す。
pub fn default_brick_layout(paddle_y: f32, cell_size: Vec2) -> BrickLayout {
    let (positions, cells) = brick_grid_candidates(paddle_y, cell_size);
    BrickLayout {
        positions,
        cell_size,
        cells,
    }
}

/// 背景画像・ブロック画像の両方から2 画像の差分をとり、自動でブロック配置を決める処理。
/// 候補グリッドの各セルについて対応する領域の平均色を比較する。
/// RGB のユークリッド距離が `BRICK_DIFF_COLOR_THRESHOLD` を超えたセルだけをブロックとして採用し、
/// パドルからアリーナ天井までの高さの `BRICK_DIFF_LAYOUT_MIN_HEIGHT_RATIO` 未満のセルは対象外にする。
/// 1 つも採用が無ければ `None`（呼び出し側で `default_brick_layout` にフォールバックする）。
pub fn diff_brick_layout(
    background_image: &Image,
    brick_image: &Image,
    paddle_y: f32,
    cell_size: Vec2,
) -> Option<BrickLayout> {
    let field = Vec2::new(RIGHT_WALL - LEFT_WALL, TOP_WALL - BOTTOM_WALL);
    let background_image_size = Vec2::new(background_image.width() as f32, background_image.height() as f32);
    let brick_image_size = Vec2::new(brick_image.width() as f32, brick_image.height() as f32);

    // とりあえず一通り、全てを網羅するようブロックを作成しておく
    let (candidate_positions, candidate_cells) = brick_grid_candidates(paddle_y, cell_size);

    let mut positions = Vec::new();
    let mut cells = Vec::new();
    for (position, cell) in candidate_positions.into_iter().zip(candidate_cells) {
        let height_ratio = (position.y - paddle_y) / (TOP_WALL - paddle_y);
        if height_ratio < BRICK_DIFF_LAYOUT_MIN_HEIGHT_RATIO {
            continue;
        }

        let background_color = match inscribed_source_rect(position, cell_size, field, background_image_size) {
            Some(rect) => average_color(background_image, rect),
            // 内接矩形の外＝黒扱い（`brick_image_rect` の黒塗りフォールバックと一貫させる）。
            None => Vec4::ZERO,
        };
        let brick_color = match inscribed_source_rect(position, cell_size, field, brick_image_size) {
            Some(rect) => average_color(brick_image, rect),
            None => Vec4::ZERO,
        };

        // アルファは比較に含めず RGB のみで差分を見る（透過対応は必要になった時点で拡張する）。
        let diff = (background_color.truncate() - brick_color.truncate()).length();
        if diff > BRICK_DIFF_COLOR_THRESHOLD {
            positions.push(position);
            cells.push(cell);
        }
    }

    if positions.is_empty() {
        return None;
    }

    Some(BrickLayout {
        positions,
        cell_size,
        cells,
    })
}
