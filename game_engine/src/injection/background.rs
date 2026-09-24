//! React（JS）から渡された背景画像の読み取りと、一時保持用 Resource。
//! Web ビルド専用（`window.__BREAKOUT_CONFIG__.backgroundBytes`）。ネイティブビルドでは
//! 常にデフォルトの背景画像にフォールバックする。`setup` からのみ使う。

use bevy::prelude::*;

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

    let config = super::breakout_config()?;

    let bytes_val = js_sys::Reflect::get(&config, &JsValue::from_str("backgroundBytes")).ok()?;
    let bytes = bytes_val.dyn_into::<js_sys::Uint8Array>().ok()?.to_vec();

    let mime = js_sys::Reflect::get(&config, &JsValue::from_str("backgroundMime"))
        .ok()
        .and_then(|v| v.as_string());

    super::decode_injected_image(&bytes, mime, "デフォルト背景を使用します")
}

/// ネイティブビルドでは JS からの注入は無い（常にデフォルト背景を使う）。
#[cfg(not(target_arch = "wasm32"))]
pub fn injected_background_image() -> Option<Image> {
    None
}
