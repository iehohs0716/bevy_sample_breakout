//! ドメイン非依存の処理を記述
//!
//! `setup` は背景画像をアリーナに内接させるのに直接使い、`common` は各ブロックへのどちらの呼び出しもドメイン型には触れない
//! 純粋な座標計算のため、特定のドメインモジュールには属させず、処理内容がそのまま名前になる
//! この独立したトップレベルファイルに置く。

use bevy::prelude::*;

/// `content`（例: 画像のピクセル寸法）を `container`（例: アリーナ）に、アスペクト比を
/// 保ったまま内接させたときの表示寸法を返す（いわゆる "contain" フィット）。
/// 比率が合わない分は余白になる（呼び出し側で黒く塗る前提）。
pub fn contain_fit(content: Vec2, container: Vec2) -> Vec2 {
    let scale = (container.x / content.x).min(container.y / content.y);
    content * scale
}

/// 「画像 1 枚を、引き伸ばさずコンテナ（例: アリーナ）いっぱいに内接表示した」と仮定したとき、
/// コンテナ座標系のある矩形領域（例: 1 個のブロックが占める範囲）が、画像のどのピクセル範囲を
/// 覆っているかを求める。呼び出し側は主に「表示上のこの範囲は、元画像のここを切り出せば描ける」
/// を知りたいときに使う（ブロックのテクスチャ切り出しや、2 画像を同じ領域で比較する差分判定など）。
///
/// # 引数（すべて同じ「コンテナ中心を原点、y 上向き」の座標系）
/// - `region_center` / `region_size`: 知りたい矩形領域の**中心座標**と**幅・高さ**
///   （例: ブロック 1 個ならその中心位置とセルサイズ）。
/// - `container`: 画像を内接させる枠の全体サイズ（例: アリーナの幅・高さ）。
/// - `image_size`: 元画像のピクセル寸法（幅・高さ）。`container` とアスペクト比が異なる場合、
///   画像は `contain_fit` で縮小され、コンテナ内に余白（レターボックス）ができる。
///
/// # 返り値
/// 画像内のピクセル矩形（`min`/`max` は画像の左上を原点とするピクセル座標）。
/// `region_center`/`region_size` の矩形が、画像を内接表示した範囲からはみ出す場合は
/// `None`（＝そこは余白で画像が存在しない。呼び出し側で黒く塗るか無視する）。
///
/// コンテナ座標は y が上向きだが画像のピクセル座標は y が下向き（左上原点）なので、
/// 内部で上下を反転させて対応づけている。
pub fn inscribed_source_rect(
    region_center: Vec2,
    region_size: Vec2,
    container: Vec2,
    image_size: Vec2,
) -> Option<Rect> {
    // 画像をコンテナに内接させたときの表示サイズ。コンテナ中心が原点なので、
    // 表示範囲は x,y ともに [-half, half]。
    let display = contain_fit(image_size, container);
    let half = display / 2.0;

    // 知りたい領域の四辺（コンテナ座標系）。
    let left = region_center.x - region_size.x / 2.0;
    let right = region_center.x + region_size.x / 2.0;
    let top = region_center.y + region_size.y / 2.0;
    let bottom = region_center.y - region_size.y / 2.0;

    // 内接表示の範囲外にはみ出す領域には対応する画像ピクセルが無い。
    if left < -half.x || right > half.x || bottom < -half.y || top > half.y {
        return None;
    }

    // 内接表示範囲（[-half, half]）の中での位置を 0..1 の割合に直し、画像のピクセル数を掛ける。
    let u_min = (left + half.x) / display.x * image_size.x;
    let u_max = (right + half.x) / display.x * image_size.x;
    // 内接表示の上端 (y=+half.y) を画像の上端 (ピクセル y=0) に対応させて上下反転する。
    let v_min = (half.y - top) / display.y * image_size.y;
    let v_max = (half.y - bottom) / display.y * image_size.y;

    Some(Rect::new(u_min, v_min, u_max, v_max))
}

/// `image` の `rect`（ピクセル矩形。`image` の範囲外にはみ出す分はクランプする）内の
/// 平均色（RGBA、各チャンネル 0..1）を返す。`rect` が空（面積 0 や範囲外）の場合は透明
/// （全チャンネル 0）を返す。
pub fn average_color(image: &Image, rect: Rect) -> Vec4 {
    let width = image.width();
    let height = image.height();

    let x_min = (rect.min.x.floor().max(0.0) as u32).min(width);
    let y_min = (rect.min.y.floor().max(0.0) as u32).min(height);
    let x_max = (rect.max.x.ceil().max(0.0) as u32).clamp(x_min, width);
    let y_max = (rect.max.y.ceil().max(0.0) as u32).clamp(y_min, height);

    let mut sum = Vec4::ZERO;
    let mut count: u32 = 0;
    for y in y_min..y_max {
        for x in x_min..x_max {
            if let Ok(color) = image.get_color_at(x, y) {
                let srgba = color.to_srgba();
                sum += Vec4::new(srgba.red, srgba.green, srgba.blue, srgba.alpha);
                count += 1;
            }
        }
    }

    if count == 0 {
        Vec4::ZERO
    } else {
        sum / count as f32
    }
}
