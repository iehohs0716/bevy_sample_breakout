//! 破れた辺（tear）のギザギザ輪郭を、中点変位法で決定的に生成するモジュール。
//!
//! ブロックの破壊面だけをギザギザに再構築する `rendering::build_brick_mesh` から使う。
//! 「同じブロック（`BrickCell`）・同じ辺なら常に同じ形」になるよう、盤面座標と辺番号から
//! 決定的な種を作り、その種で駆動する疑似乱数を中点変位法に食わせる。

use bevy::prelude::*;

use crate::components::BrickCell;
use crate::config::{TEAR_DEPTH, TEAR_ROUGHNESS};

/// 決定的な疑似乱数（xorshift32）。同じブロック・同じ辺は毎回同じギザギザになり再現性がある。
struct TearRng(u32);

impl TearRng {
    fn new(seed: u32) -> Self {
        Self(seed | 1) // 0 だとxorshiftが退化するので奇数化
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    fn next_unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// ブロックの盤面座標と辺番号から決定的な種を作る。同じセル・同じ辺なら常に同じギザギザになる。
pub fn seed_for(cell: BrickCell, edge_index: u32) -> u32 {
    let r = cell.row as u32;
    let c = cell.col as u32;
    r.wrapping_mul(73856093)
        ^ c.wrapping_mul(19349663)
        ^ edge_index.wrapping_mul(83492791)
        ^ 0x9E3779B9
}

/// 中点変位法。`a`→`b` の辺の間に、`depth` 段まで再帰的に変位点を差し込んで `out` に積む
/// （`a` 自身と `b` 自身は積まない＝呼び出し側が両端を管理する前提）。再帰ごとに辺長が半分に
/// なるので振れ幅（`amplitude`）も自動的に減衰する。
fn midpoint_displace(a: Vec2, b: Vec2, depth: u32, roughness: f32, rng: &mut TearRng, out: &mut Vec<Vec2>) {
    if depth == 0 {
        return;
    }
    let mid = (a + b) / 2.0;
    let edge = b - a;
    let normal = Vec2::new(-edge.y, edge.x).normalize_or_zero();
    let amplitude = edge.length() * roughness;
    let offset = (rng.next_unit() * 2.0 - 1.0) * amplitude;
    let displaced = mid + normal * offset;

    midpoint_displace(a, displaced, depth - 1, roughness, rng, out);
    out.push(displaced);
    midpoint_displace(displaced, b, depth - 1, roughness, rng, out);
}

/// `start`→`end` の辺を破れたギザギザ輪郭に変え、両端を除く変位点だけを `out` に積む
/// （両端 `start`/`end` は呼び出し側が管理する前提）。`cell`・`edge_index` から決定的な種を
/// 作るので、同じセル・同じ辺なら常に同じ形になる。振れ幅・分割段数は `config` の
/// `TEAR_ROUGHNESS` / `TEAR_DEPTH` に従う。
pub fn push_torn_edge(cell: BrickCell, edge_index: u32, start: Vec2, end: Vec2, out: &mut Vec<Vec2>) {
    let mut rng = TearRng::new(seed_for(cell, edge_index));
    midpoint_displace(start, end, TEAR_DEPTH, TEAR_ROUGHNESS, &mut rng, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 同じセル・同じ辺なら常に同じギザギザになる（決定的）ことを確認する。
    #[test]
    fn seed_for_is_deterministic_per_cell_and_edge() {
        let cell = BrickCell { row: 2, col: 9 };
        assert_eq!(seed_for(cell, 0), seed_for(cell, 0));
        assert_ne!(seed_for(cell, 0), seed_for(cell, 1), "辺が違えばseedも違うはず");
        assert_ne!(
            seed_for(cell, 0),
            seed_for(BrickCell { row: 9, col: 2 }, 0),
            "セルが違えばseedも違うはず"
        );
    }
}
