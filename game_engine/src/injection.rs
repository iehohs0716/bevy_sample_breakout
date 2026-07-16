//! React（JS）から `window.__BREAKOUT_CONFIG__` 経由で渡された初期化パラメータ
//! （背景画像・初期ブロック配置・ブロック用画像）の読み取りと、それを一時保持する Resource。
//!
//! これにより「アプリのコード（Rust/WASM）は 1 ビルド」のまま、サービスごとに
//! 背景やブロックの配置・絵柄を React 側から差し替えられる。いずれも Web ビルド専用で、
//! ネイティブビルドでは常にデフォルトへフォールバックする。

use bevy::prelude::*;

use crate::config::{
    BRICK_SIZE, GAP_BETWEEN_BRICKS, GAP_BETWEEN_BRICKS_AND_CEILING, GAP_BETWEEN_BRICKS_AND_SIDES,
    GAP_BETWEEN_PADDLE_AND_BRICKS, LEFT_WALL, RIGHT_WALL, TOP_WALL,
};

/// Web ビルド専用。`window.__BREAKOUT_CONFIG__` を取得する。
/// 未定義 / null の場合は `None`。
#[cfg(target_arch = "wasm32")]
fn breakout_config() -> Option<wasm_bindgen::JsValue> {
    use wasm_bindgen::JsValue;

    let window = web_sys::window()?;
    let config = js_sys::Reflect::get(&window, &JsValue::from_str("__BREAKOUT_CONFIG__")).ok()?;
    if config.is_undefined() || config.is_null() {
        return None;
    }
    Some(config)
}

/// Web ビルド専用。画像バイト列と（任意の）MIME を `Image` にデコードする共通処理。
/// MIME が受け取れればそれを使い、無ければ拡張子 png とみなす。
/// デコードに失敗した場合は `fallback_desc`（例: 「デフォルト背景を使用します」）を
/// 添えて warn し、`None` を返す。
#[cfg(target_arch = "wasm32")]
fn decode_injected_image(bytes: &[u8], mime: Option<String>, fallback_desc: &str) -> Option<Image> {
    use bevy::{
        asset::RenderAssetUsages,
        image::{CompressedImageFormats, ImageSampler, ImageType},
    };

    if bytes.is_empty() {
        return None;
    }

    // 画像フォーマットは MIME で受け取れれば使い、無ければ拡張子 png とみなす。
    let image_type = match mime.as_deref() {
        Some(m) if !m.is_empty() => ImageType::MimeType(m),
        _ => ImageType::Extension("png"),
    };

    match Image::from_buffer(
        bytes,
        image_type,
        CompressedImageFormats::NONE,
        true,
        ImageSampler::Default,
        RenderAssetUsages::default(),
    ) {
        Ok(image) => Some(image),
        Err(err) => {
            warn!("画像のデコードに失敗しました。{fallback_desc}: {err}");
            None
        }
    }
}

// React（JS）から渡された背景画像を一時的に保持する Resource。
// `setup` で取り出して `Assets<Image>` に登録し、背景スプライトに使う。
// `None` の場合は `BACKGROUND_IMAGE_PATH` のデフォルト画像にフォールバックする。
#[derive(Resource, Default)]
pub struct BackgroundOverride(pub Option<Image>);

/// Web ビルド専用。`window.__BREAKOUT_CONFIG__.backgroundBytes`（React が fetch した
/// 画像バイト列 = Uint8Array）を読み、`Image` にデコードして返す。
/// 設定が無い / 読めない / デコード失敗の場合は `None`（デフォルト背景にフォールバック）。
#[cfg(target_arch = "wasm32")]
pub fn injected_background_image() -> Option<Image> {
    use wasm_bindgen::{JsCast, JsValue};

    let config = breakout_config()?;

    let bytes_val = js_sys::Reflect::get(&config, &JsValue::from_str("backgroundBytes")).ok()?;
    let bytes = bytes_val.dyn_into::<js_sys::Uint8Array>().ok()?.to_vec();

    let mime = js_sys::Reflect::get(&config, &JsValue::from_str("backgroundMime"))
        .ok()
        .and_then(|v| v.as_string());

    decode_injected_image(&bytes, mime, "デフォルト背景を使用します")
}

/// ネイティブビルドでは JS からの注入は無い（常にデフォルト背景を使う）。
#[cfg(not(target_arch = "wasm32"))]
pub fn injected_background_image() -> Option<Image> {
    None
}

// React（JS）から渡された「初期ブロック配置」。座標は Bevy ワールド座標
// （中心原点・y 上向き・1 単位 = 1px。アリーナは x∈[LEFT_WALL, RIGHT_WALL],
// y∈[BOTTOM_WALL, TOP_WALL]）で、各ブロックの *中心* 位置を表す。
// `cell_size` は全ブロック共通のセルの大きさ（幅・高さ）。
pub struct BrickLayout {
    pub positions: Vec<Vec2>,
    pub cell_size: Vec2,
}

/// React（JS）由来の配置が無いときに使う、アリーナを敷き詰めるデフォルトのブロック配置を計算して返す。
/// `injected_brick_layout()`（外部指定）と対になる「組み込みの標準配置」。`paddle_y` はパドルの
/// 中心 y で、ブロック帯はその上に `GAP_BETWEEN_PADDLE_AND_BRICKS` だけ空けて始まる。
/// 入力は全てコンパイル時定数由来なので結果は決定的（起動時に一度計算するだけ）。
pub fn default_brick_layout(paddle_y: f32) -> BrickLayout {
    let total_width_of_bricks = (RIGHT_WALL - LEFT_WALL) - 2. * GAP_BETWEEN_BRICKS_AND_SIDES;
    let bottom_edge_of_bricks = paddle_y + GAP_BETWEEN_PADDLE_AND_BRICKS;
    let total_height_of_bricks = TOP_WALL - bottom_edge_of_bricks - GAP_BETWEEN_BRICKS_AND_CEILING;

    assert!(total_width_of_bricks > 0.0);
    assert!(total_height_of_bricks > 0.0);

    // 使える面積に何行・何列入るか（切り捨て）。
    let n_columns = (total_width_of_bricks / (BRICK_SIZE.x + GAP_BETWEEN_BRICKS)).floor() as usize;
    let n_rows = (total_height_of_bricks / (BRICK_SIZE.y + GAP_BETWEEN_BRICKS)).floor() as usize;
    let n_vertical_gaps = n_columns - 1;

    // 列数を丸めたぶん帯の幅は領域より狭くなるので、中心から帯の半分だけ戻して中央揃えにする。
    let center_of_bricks = (LEFT_WALL + RIGHT_WALL) / 2.0;
    let left_edge_of_bricks = center_of_bricks
        - (n_columns as f32 / 2.0 * BRICK_SIZE.x)
        - n_vertical_gaps as f32 / 2.0 * GAP_BETWEEN_BRICKS;

    // `translation` は中心座標なので、左下の縁から半セルぶん内側を最初のブロックの中心にする。
    let offset_x = left_edge_of_bricks + BRICK_SIZE.x / 2.;
    let offset_y = bottom_edge_of_bricks + BRICK_SIZE.y / 2.;

    let mut positions = Vec::with_capacity(n_rows * n_columns);
    for row in 0..n_rows {
        for column in 0..n_columns {
            positions.push(Vec2::new(
                offset_x + column as f32 * (BRICK_SIZE.x + GAP_BETWEEN_BRICKS),
                offset_y + row as f32 * (BRICK_SIZE.y + GAP_BETWEEN_BRICKS),
            ));
        }
    }

    BrickLayout {
        positions,
        cell_size: BRICK_SIZE,
    }
}

// React（JS）から渡された初期ブロック配置を一時的に保持する Resource。
// `setup` で取り出してブロックを spawn する。`None` の場合は従来どおり
// アリーナを敷き詰めるデフォルト配置にフォールバックする。
#[derive(Resource, Default)]
pub struct BrickLayoutOverride(pub Option<BrickLayout>);

/// Web ビルド専用。`window.__BREAKOUT_CONFIG__.bricks`（`[{x, y}, ...]` の配列）と
/// `.cellSize`（`{width, height}`）を読み、初期ブロック配置として返す。
/// - `bricks` が無い / 空 / 各要素に x,y が無い場合は `None`（デフォルト配置にフォールバック）。
/// - `cellSize` が無い / 不正な場合はデフォルトの `BRICK_SIZE` を使う。
#[cfg(target_arch = "wasm32")]
pub fn injected_brick_layout() -> Option<BrickLayout> {
    use wasm_bindgen::{JsCast, JsValue};

    let config = breakout_config()?;

    let bricks_val = js_sys::Reflect::get(&config, &JsValue::from_str("bricks")).ok()?;
    let bricks_arr = bricks_val.dyn_into::<js_sys::Array>().ok()?;
    if bricks_arr.length() == 0 {
        return None;
    }

    let mut positions = Vec::with_capacity(bricks_arr.length() as usize);
    for i in 0..bricks_arr.length() {
        let brick = bricks_arr.get(i);
        let x = js_sys::Reflect::get(&brick, &JsValue::from_str("x"))
            .ok()
            .and_then(|v| v.as_f64());
        let y = js_sys::Reflect::get(&brick, &JsValue::from_str("y"))
            .ok()
            .and_then(|v| v.as_f64());
        match (x, y) {
            (Some(x), Some(y)) => positions.push(Vec2::new(x as f32, y as f32)),
            _ => warn!("ブロック配置の要素 {i} に数値の x/y が無いためスキップします"),
        }
    }
    if positions.is_empty() {
        return None;
    }

    // セルの大きさ。指定が無い / 不正な場合はデフォルトの BRICK_SIZE にフォールバック。
    let cell_size = js_sys::Reflect::get(&config, &JsValue::from_str("cellSize"))
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null())
        .and_then(|cell| {
            let w = js_sys::Reflect::get(&cell, &JsValue::from_str("width"))
                .ok()
                .and_then(|v| v.as_f64());
            let h = js_sys::Reflect::get(&cell, &JsValue::from_str("height"))
                .ok()
                .and_then(|v| v.as_f64());
            match (w, h) {
                (Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some(Vec2::new(w as f32, h as f32)),
                _ => None,
            }
        })
        .unwrap_or(BRICK_SIZE);

    Some(BrickLayout {
        positions,
        cell_size,
    })
}

/// ネイティブビルドでは JS からの注入は無い（常にデフォルト配置を使う）。
#[cfg(not(target_arch = "wasm32"))]
pub fn injected_brick_layout() -> Option<BrickLayout> {
    None
}

// React（JS）から渡された「ブロック用の画像」を一時的に保持する Resource。
// `setup` で `Assets<Image>` に登録し、各ブロックが自分の位置に対応する領域を切り出して使う。
// `None` の場合は `BRICK_COLOR` の単色ブロックにフォールバックする。
#[derive(Resource, Default)]
pub struct BrickImageOverride(pub Option<Image>);

/// Web ビルド専用。`window.__BREAKOUT_CONFIG__.brickImage`
/// （`{ bytes: Uint8Array, mime?: string }`）を読み、デコード済みの `Image` を返す。
/// 設定が無い / 読めない / デコード失敗の場合は `None`（単色ブロックにフォールバック）。
#[cfg(target_arch = "wasm32")]
pub fn injected_brick_image() -> Option<Image> {
    use wasm_bindgen::{JsCast, JsValue};

    let config = breakout_config()?;

    let entry = js_sys::Reflect::get(&config, &JsValue::from_str("brickImage")).ok()?;
    if entry.is_undefined() || entry.is_null() {
        return None;
    }

    let bytes = js_sys::Reflect::get(&entry, &JsValue::from_str("bytes"))
        .ok()?
        .dyn_into::<js_sys::Uint8Array>()
        .ok()?
        .to_vec();

    let mime = js_sys::Reflect::get(&entry, &JsValue::from_str("mime"))
        .ok()
        .and_then(|v| v.as_string());

    decode_injected_image(&bytes, mime, "単色ブロックを使用します")
}

/// ネイティブビルドでは JS からの注入は無い（常に単色ブロックを使う）。
#[cfg(not(target_arch = "wasm32"))]
pub fn injected_brick_image() -> Option<Image> {
    None
}
