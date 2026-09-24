//! React（JS）から渡された「ブロック用の画像」の読み取りと、一時保持用 Resource。
//! Web ビルド専用（`window.__BREAKOUT_CONFIG__.brickImage`）。ネイティブビルドでは常に
//! 単色ブロックにフォールバックする。`setup` からのみ使う。

use bevy::prelude::*;

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

    let config = super::breakout_config()?;

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

    super::decode_injected_image(&bytes, mime, "単色ブロックを使用します")
}

/// ネイティブビルドでは JS からの注入は無い（常に単色ブロックを使う）。
#[cfg(not(target_arch = "wasm32"))]
pub fn injected_brick_image() -> Option<Image> {
    None
}
