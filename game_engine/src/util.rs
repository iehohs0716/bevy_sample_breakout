//! ドメイン非依存の処理を記述
//!
//! `setup` は背景画像をアリーナに内接させるのに直接使い、`common` は各ブロックへのどちらの呼び出しもドメイン型には触れない
//! 純粋な座標計算のため、特定のドメインモジュールには属させず、処理内容がそのまま名前になる
//! この独立したトップレベルファイルに置く。

use bevy::prelude::*;

/// `content`（例: 画像のピクセル寸法）を `container`（例: アリーナ）に、アスペクト比を
/// 保ったまま内接させたときの表示寸法を返す（いわゆる "contain" フィット）。
/// 比率が合わない分は余白になる（呼び出し側で黒く塗る前提）。
pub fn contain_fit(content: Vec2, container: Vec2) -> (Vec2, f32) {
    let scale = (container.x / content.x).min(container.y / content.y);
    (content * scale, scale)
}

/// ブロックなど、ゲーム画面上のオブジェクトの位置（`region_center`）とサイズ（`region_size`）を指定すると、
/// それが背景画像の「どのピクセル範囲」に乗っているかを計算して返す関数です（テクスチャの切り出しなどに使用）。
/// 
/// 画像はコンテナに対して「アスペクト比維持・上部中央寄せ」で内接配置される前提で計算され、
/// 画面座標（Y上向き）から画像ピクセル座標（Y下向き・左上原点）への変換も行う。
///
/// # 返り値
/// 画像内のピクセル矩形。指定領域が画像からはみ出している場合は `None` を返す。
pub fn inscribed_source_rect(
    region_center: Vec2,
    region_size: Vec2,
    container: Vec2,
    image_size: Vec2,
) -> Option<Rect> {
    let (display, scale) = contain_fit(image_size, container);

    // セルサイズの整数倍に高さを収めるための端数削り（トップクロップ）
    let remainder_y = display.y % region_size.y;
    let cropped_display_y = display.y - remainder_y;
    let crop_texture_pixels_top = remainder_y / scale;

    let half_x = display.x / 2.0;
    let top_y = container.y / 2.0;
    let bottom_y = top_y - cropped_display_y;

    // 知りたい領域の四辺（コンテナ座標系）。
    let left = region_center.x - region_size.x / 2.0;
    let right = region_center.x + region_size.x / 2.0;
    let top = region_center.y + region_size.y / 2.0;
    let bottom = region_center.y - region_size.y / 2.0;

    // 内接表示の範囲外にはみ出す領域には対応する画像ピクセルが無い。
    if left < -half_x || right > half_x || bottom < bottom_y || top > top_y {
        return None;
    }

    // 内接表示範囲の中での位置を 0..1 の割合に直し、画像のピクセル数を掛ける。
    let u_min = ((left + half_x) / display.x * image_size.x).clamp(0.0, image_size.x);
    let u_max = ((right + half_x) / display.x * image_size.x).clamp(0.0, image_size.x);

    let usable_texture_height = image_size.y - crop_texture_pixels_top;
    let v_min = (crop_texture_pixels_top
        + (top_y - top) / cropped_display_y * usable_texture_height)
        .clamp(0.0, image_size.y);
    let v_max = (crop_texture_pixels_top
        + (top_y - bottom) / cropped_display_y * usable_texture_height)
        .clamp(0.0, image_size.y);

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
