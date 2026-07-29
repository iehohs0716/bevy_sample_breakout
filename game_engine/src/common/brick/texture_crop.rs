//! 1 枚の画像を比率維持でアリーナに貼ったと仮定したときに、各ブロックが覆う領域に対応する
//! 画像内の切り出し矩形（0..1 の UV）を計算する。`spawn_brick`（親モジュール）からのみ使う。

use bevy::prelude::*;

use crate::config::{BOTTOM_WALL, LEFT_WALL, RIGHT_WALL, TOP_WALL};
use crate::util::contain_fit;

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

/// ブロックが覆う領域に対応する、画像内の 0..1 UV 矩形を返す。内接矩形からはみ出す場合は
/// `None`（呼び出し側で黒く塗る）。`brick_image_rect` と `normalize_rect` は常にこの順で
/// セットで使われるため、1 つの関数にまとめて公開する。
pub(super) fn brick_uv_rect(position: Vec2, size: Vec2, image_size: Vec2) -> Option<Rect> {
    let rect = brick_image_rect(position, size, image_size)?;
    Some(normalize_rect(rect, image_size))
}
