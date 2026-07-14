//! React（JS）から `window.__BREAKOUT_CONFIG__` 経由で渡された初期化パラメータ
//! （背景画像・初期ブロック配置・ブロック用画像）の読み取りと、それを一時保持する Resource。
//!
//! これにより「アプリのコード（Rust/WASM）は 1 ビルド」のまま、サービスごとに
//! 背景やブロックの配置・絵柄を React 側から差し替えられる。いずれも Web ビルド専用で、
//! ネイティブビルドでは常にデフォルトへフォールバックする。

use bevy::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::config::BRICK_SIZE;

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
    use bevy::{
        asset::RenderAssetUsages,
        image::{CompressedImageFormats, ImageSampler, ImageType},
    };
    use wasm_bindgen::{JsCast, JsValue};

    let window = web_sys::window()?;
    let config = js_sys::Reflect::get(&window, &JsValue::from_str("__BREAKOUT_CONFIG__")).ok()?;
    if config.is_undefined() || config.is_null() {
        return None;
    }

    let bytes_val = js_sys::Reflect::get(&config, &JsValue::from_str("backgroundBytes")).ok()?;
    let bytes = bytes_val.dyn_into::<js_sys::Uint8Array>().ok()?.to_vec();
    if bytes.is_empty() {
        return None;
    }

    // 画像フォーマットは MIME で受け取れれば使い、無ければ拡張子 png とみなす。
    let mime = js_sys::Reflect::get(&config, &JsValue::from_str("backgroundMime"))
        .ok()
        .and_then(|v| v.as_string());
    let image_type = match mime.as_deref() {
        Some(m) if !m.is_empty() => ImageType::MimeType(m),
        _ => ImageType::Extension("png"),
    };

    match Image::from_buffer(
        &bytes,
        image_type,
        CompressedImageFormats::NONE,
        true,
        ImageSampler::Default,
        RenderAssetUsages::default(),
    ) {
        Ok(image) => Some(image),
        Err(err) => {
            warn!("背景画像のデコードに失敗しました。デフォルト背景を使用します: {err}");
            None
        }
    }
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

    let window = web_sys::window()?;
    let config = js_sys::Reflect::get(&window, &JsValue::from_str("__BREAKOUT_CONFIG__")).ok()?;
    if config.is_undefined() || config.is_null() {
        return None;
    }

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

// React（JS）から渡された「ブロック用の画像（2 種類想定）」を一時的に保持する Resource。
// `setup` で `Assets<Image>` に登録してハンドル列に変換し、各ブロックへ順番に割り当てる。
// 空の場合は従来どおり `BRICK_COLOR` の単色ブロックにフォールバックする。
#[derive(Resource, Default)]
pub struct BrickImagesOverride(pub Vec<Image>);

/// Web ビルド専用。`window.__BREAKOUT_CONFIG__.brickImages`
/// （`[{ bytes: Uint8Array, mime?: string }, ...]` の配列）を読み、
/// デコード済みの `Image` 列として返す。ブロックにはこの配列を先頭から順に
/// （個数を超えたら折り返して）割り当てる。
/// 設定が無い / 空 / 全てデコード失敗の場合は空 Vec（単色ブロックにフォールバック）。
#[cfg(target_arch = "wasm32")]
pub fn injected_brick_images() -> Vec<Image> {
    use bevy::{
        asset::RenderAssetUsages,
        image::{CompressedImageFormats, ImageSampler, ImageType},
    };
    use wasm_bindgen::{JsCast, JsValue};

    let mut result = Vec::new();

    let Some(window) = web_sys::window() else {
        return result;
    };
    let Ok(config) = js_sys::Reflect::get(&window, &JsValue::from_str("__BREAKOUT_CONFIG__"))
    else {
        return result;
    };
    if config.is_undefined() || config.is_null() {
        return result;
    }
    let Ok(images_val) = js_sys::Reflect::get(&config, &JsValue::from_str("brickImages")) else {
        return result;
    };
    let Ok(images_arr) = images_val.dyn_into::<js_sys::Array>() else {
        return result;
    };

    for i in 0..images_arr.length() {
        let entry = images_arr.get(i);
        let bytes = match js_sys::Reflect::get(&entry, &JsValue::from_str("bytes"))
            .ok()
            .and_then(|v| v.dyn_into::<js_sys::Uint8Array>().ok())
        {
            Some(arr) => arr.to_vec(),
            None => {
                warn!("ブロック画像の要素 {i} に bytes(Uint8Array) が無いためスキップします");
                continue;
            }
        };
        if bytes.is_empty() {
            continue;
        }

        // 画像フォーマットは MIME で受け取れれば使い、無ければ拡張子 png とみなす。
        let mime = js_sys::Reflect::get(&entry, &JsValue::from_str("mime"))
            .ok()
            .and_then(|v| v.as_string());
        let image_type = match mime.as_deref() {
            Some(m) if !m.is_empty() => ImageType::MimeType(m),
            _ => ImageType::Extension("png"),
        };

        match Image::from_buffer(
            &bytes,
            image_type,
            CompressedImageFormats::NONE,
            true,
            ImageSampler::Default,
            RenderAssetUsages::default(),
        ) {
            Ok(image) => result.push(image),
            Err(err) => warn!("ブロック画像のデコードに失敗しました（要素 {i}）: {err}"),
        }
    }

    result
}

/// ネイティブビルドでは JS からの注入は無い（常に単色ブロックを使う）。
#[cfg(not(target_arch = "wasm32"))]
pub fn injected_brick_images() -> Vec<Image> {
    Vec::new()
}
