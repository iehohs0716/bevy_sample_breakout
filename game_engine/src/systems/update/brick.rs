//! ブロック単一ドメインの system と observer（全ブロック消滅判定・破壊時の隣接辺更新・
//! 破れたブロックの再描画）。`Query` を取る実際の Bevy system はドメインを問わずここに置き、
//! `spawn_brick` のような手動呼び出しのヘルパー関数（`common::brick`）とは区別する。

use bevy::prelude::*;

use crate::common::build_brick_mesh;
use crate::components::{Brick, BrickDestroyed, BrokenEdges, GameState};
use crate::config::BRICK_OUTLINE_COLOR;

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

/// 各ブロックの境界をうっすら描画する。無傷（未破壊）のブロック同士はメッシュの見た目が
/// 繋がって1枚の絵に見えてしまう（画像を貼った盤面の「窓」がシームレスに並ぶため）ので、
/// 低アルファの矩形輪郭を重ねて描き、個々のブロックの位置・大きさを視認できるようにする
/// （毎フレーム描き直すデバッグ線であり、`Brick` のメッシュ自体は変更しない）。
pub fn draw_brick_outlines(mut gizmos: Gizmos, bricks: Query<(&Transform, &Brick)>) {
    for (transform, brick) in &bricks {
        gizmos.rect_2d(transform.translation.truncate(), brick.size, BRICK_OUTLINE_COLOR);
    }
}

/// 全ブロックが無くなったらクリア状態へ遷移する。`Playing` 中のみ動作させる想定
/// （ブロックは `Startup` で spawn 済みなので、最初の `Update` フレームには存在する）。
/// 実際の JS 通知は状態遷移側（`OnEnter(GameState::Cleared)` → `on_game_clear`）で行う。
pub fn check_game_clear(
    bricks: Query<(), With<Brick>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if bricks.is_empty() {
        next_state.set(GameState::Cleared);
    }
}

/// ブロックが破壊された時、隣接する4方向(上下左右)のブロックのうち、破壊されたブロックと
/// 接していた辺だけを「破れた境界」にする。かつて隣接ブロックが存在しなかった辺(壁際・隙間)は
/// この経路を一切通らないので、破れた見た目にならない。
pub fn mark_broken_edges_on_brick_destroyed(
    trigger: On<BrickDestroyed>,
    mut bricks: Query<(&Brick, &mut BrokenEdges)>,
) {
    let destroyed = trigger.cell;
    for (brick, mut broken) in &mut bricks {
        let cell = brick.cell;
        if cell.row == destroyed.row + 1 && cell.col == destroyed.col {
            broken.bottom = true; // 自分は破壊されたセルの真上 → 自分の下辺が破れる
        } else if cell.row == destroyed.row - 1 && cell.col == destroyed.col {
            broken.top = true; // 真下 → 上辺が破れる
        } else if cell.col == destroyed.col + 1 && cell.row == destroyed.row {
            broken.left = true; // 右隣 → 左辺が破れる
        } else if cell.col == destroyed.col - 1 && cell.row == destroyed.row {
            broken.right = true; // 左隣 → 右辺が破れる
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{BrickCell, BrickFill};

    fn cell(row: i32, col: i32) -> BrickCell {
        BrickCell { row, col }
    }

    fn spawn_cell(world: &mut World, row: i32, col: i32) -> Entity {
        world
            .spawn((
                Brick {
                    size: Vec2::ONE,
                    cell: cell(row, col),
                    fill: BrickFill::Color(Color::WHITE),
                },
                BrokenEdges::default(),
            ))
            .id()
    }

    fn broken(world: &World, entity: Entity) -> BrokenEdges {
        *world.get::<BrokenEdges>(entity).unwrap()
    }

    /// `mark_broken_edges_on_brick_destroyed` が row/col の各方向を正しい辺に対応させることを
    /// 固定化する回帰テスト。CLAUDE.md の規約(row 増加=上, col 増加=右)を前提に、
    /// 「真上のブロックの下辺」「真下の上辺」「右隣の左辺」「左隣の右辺」が破れ、
    /// それ以外(隣接しないセル)は変化しないことを確認する。
    #[test]
    fn marks_only_the_edge_facing_the_destroyed_neighbor() {
        let mut world = World::new();
        world.add_observer(mark_broken_edges_on_brick_destroyed);

        let above = spawn_cell(&mut world, 1, 0);
        let below = spawn_cell(&mut world, -1, 0);
        let right = spawn_cell(&mut world, 0, 1);
        let left = spawn_cell(&mut world, 0, -1);
        let unrelated = spawn_cell(&mut world, 5, 5);
        let diagonal = spawn_cell(&mut world, 1, 1);

        world.trigger(BrickDestroyed { cell: cell(0, 0) });

        assert_eq!(
            broken(&world, above),
            BrokenEdges { bottom: true, ..Default::default() },
            "真上のブロックは下辺が破れるはず"
        );
        assert_eq!(
            broken(&world, below),
            BrokenEdges { top: true, ..Default::default() },
            "真下のブロックは上辺が破れるはず"
        );
        assert_eq!(
            broken(&world, right),
            BrokenEdges { left: true, ..Default::default() },
            "右隣のブロックは左辺が破れるはず"
        );
        assert_eq!(
            broken(&world, left),
            BrokenEdges { right: true, ..Default::default() },
            "左隣のブロックは右辺が破れるはず"
        );
        assert_eq!(
            broken(&world, unrelated),
            BrokenEdges::default(),
            "隣接しないブロックは変化しないはず"
        );
        assert_eq!(
            broken(&world, diagonal),
            BrokenEdges::default(),
            "斜めに隣接するだけのブロックは辺を共有しないので変化しないはず"
        );
    }
}
