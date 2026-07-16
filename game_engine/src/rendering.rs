//! 画像のフィット計算とブロックの描画（spawn）ヘルパー。
//!
//! 画像は「引き伸ばし（テクスチャ漬け）」ではなく、盤面に比率維持で貼った 1 枚の絵として扱い、
//! 各ブロックはその絵のうち自分が覆う領域だけを切り出して表示する。全ブロックが揃うと 1 枚の
//! 絵になり、ブロックを壊すとその穴から背後の背景画像が見える。

use bevy::prelude::*;

use crate::components::{Brick, Collider};
use crate::config::{BOTTOM_WALL, BRICK_COLOR, LEFT_WALL, RIGHT_WALL, TOP_WALL};

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

/// 1 つのブロックを spawn する。`position` はワールド座標での中心、`size` はセルの大きさ。
/// `image` が `Some` なら、比率維持で貼った画像のうちこのブロックが覆う領域だけを切り出して
/// 表示する（引き伸ばしではなく「そのまま貼った絵の一部分」）。内接矩形の外や画像未指定なら
/// それぞれ黒・単色で描く。デフォルト配置と JS 注入配置の双方から使い、spawn ロジックを一本化する。
pub fn spawn_brick(
    commands: &mut Commands,
    position: Vec2,
    size: Vec2,
    image: Option<(Handle<Image>, Vec2)>,
) {
    // 画像・単色いずれも基準サイズを 1x1 にし、Transform.scale = セルサイズで拡大する
    // （壁・パドルと同じ方式）。画像スプライトは custom_size 未指定だと画像本来の
    // ピクセル寸法が基準になり、さらに scale が掛かって巨大化するため Vec2::ONE を明示する。
    // `rect` で画像内の切り出し領域を指定し、それを 1x1 の矩形に描く。
    let sprite = match image {
        Some((handle, image_size)) => match brick_image_rect(position, size, image_size) {
            Some(rect) => Sprite {
                image: handle,
                custom_size: Some(Vec2::ONE),
                rect: Some(rect),
                ..default()
            },
            // 内接矩形の外にあるブロックは黒（＝画像の余白と同じ扱い）。
            None => Sprite {
                color: Color::BLACK,
                ..default()
            },
        },
        None => Sprite {
            color: BRICK_COLOR,
            ..default()
        },
    };

    commands.spawn((
        sprite,
        Transform {
            translation: position.extend(0.0),
            scale: size.extend(1.0),
            ..default()
        },
        Brick,
        Collider,
    ));
}
