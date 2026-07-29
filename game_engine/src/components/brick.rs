//! ブロック関連の Component / Event（`Brick` / `BrickCell` / `BrokenEdges` / `BrickFill` /
//! `BrickDestroyed`）の定義。

use bevy::prelude::*;

/// ブロック本体。「これはブロックだ」という識別マーカーを兼ね、spawn 時に確定して以後
/// 変化しない不変データ（大きさ・盤面座標・塗り方）をまとめて保持する。実行中に変化する
/// 「破れた辺」だけは `BrokenEdges` として別コンポーネントに分け、`Changed<BrokenEdges>`
/// で対象だけを再描画できるようにしている（可変で変更検知したいものだけを切り出す方針）。
/// `fill` が `Handle<Image>` を含むため `Copy` にはできず `Clone` のみ。
#[derive(Component, Clone)]
pub struct Brick {
    pub size: Vec2,
    /// 盤面上の格子座標(行・列)。隣接ブロック破壊時にどの辺が「元は接していたが今は空洞と
    /// 接する面になった」かを判定する基準であり、ギザギザ形状の決定的シードにも使う。
    pub cell: BrickCell,
    /// 塗り方。破れた辺の再描画(メッシュ再構築)時にも Sprite 時代と同じ見た目の規則
    /// (画像切り出し優先、範囲外は黒、指定無しは単色)を再現するために保持する。
    pub fill: BrickFill,
}

/// ブロックの盤面上の格子座標(行・列)。`Brick` のフィールドとして持つ値型（それ単独では
/// コンポーネントにしない）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BrickCell {
    pub row: i32,
    pub col: i32,
}

/// 上下左右それぞれの辺が「破れた境界」かどうか。true の辺だけ中点変位法のギザギザ輪郭で
/// 再描画する。一度も隣接ブロックが存在しなかった辺(アリーナ端や配置上の隙間)はここに
/// 反映されない(破れは「かつてブロックが接していた面」限定の見た目のため)。
/// 実行中に変化する唯一のブロック状態なので、`Brick` に含めず独立コンポーネントにして
/// `Changed<BrokenEdges>` による絞り込み再描画を効かせる。
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BrokenEdges {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

/// ブロックの塗り方。`Brick.fill` として持つ値型（それ単独ではコンポーネントにしない）。
#[derive(Clone)]
pub enum BrickFill {
    /// `image` のうち `uv_rect`(画像全体を 0..1 に正規化した UV 矩形)を貼る。
    Textured { image: Handle<Image>, uv_rect: Rect },
    Color(Color),
}

/// ブロックが破壊された直後に発火する内部イベント。隣接ブロックへ「破れた境界」を
/// 伝える `mark_broken_edges_on_brick_destroyed`(systems.rs)が拾う。
#[derive(Event)]
pub struct BrickDestroyed {
    pub cell: BrickCell,
}
