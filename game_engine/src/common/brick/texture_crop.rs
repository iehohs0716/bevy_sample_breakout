//! 1 枚の画像を比率維持でアリーナに貼ったと仮定したときに、各ブロックが覆う領域に対応する
//! 画像内の切り出し矩形（0..1 の UV）を計算する。`spawn_brick`（親モジュール）からのみ使う。

use bevy::prelude::*;

use crate::config::{BOTTOM_WALL, LEFT_WALL, RIGHT_WALL, TOP_WALL};
use crate::util::inscribed_source_rect;

/// 画像をアリーナに contain フィット（比率維持で内接・中央寄せ）で「そのまま」貼ったと仮定し、
/// `position` を中心・`size` を大きさとするブロックが覆う領域に対応する画像内の切り出し矩形
/// （ピクセル）を返す。ブロックが表示領域（内接矩形）からはみ出す場合は `None`（＝黒くする）。
/// 全ブロックが揃うと 1 枚の絵になり、ブロックを壊すとその穴から背後の背景画像が見える。
/// アリーナ矩形を固定した `util::inscribed_source_rect` の薄いラッパー（`injection` の画像差分
/// 判定も同じ写像を使うため、幾何計算自体は `util` 側に集約している）。
fn brick_image_rect(position: Vec2, size: Vec2, image_size: Vec2) -> Option<Rect> {
    let field = Vec2::new(RIGHT_WALL - LEFT_WALL, TOP_WALL - BOTTOM_WALL);
    inscribed_source_rect(position, size, field, image_size)
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

/// ブロックが覆う領域に対応する、画像内の 0..1 UV 矩形を返す。内接矩形からはみ出す場合は
/// `None`（呼び出し側で黒く塗る）。`brick_image_rect` と `normalize_rect` は常にこの順で
/// セットで使われるため、1 つの関数にまとめて公開する。
pub(super) fn brick_uv_rect(position: Vec2, size: Vec2, image_size: Vec2) -> Option<Rect> {
    let rect = brick_image_rect(position, size, image_size)?;
    Some(normalize_rect(rect, image_size))
}
