//! React（JS）から `window.__BREAKOUT_CONFIG__` 経由で渡された初期化パラメータの読み取りと、
//! それを一時保持する Resource。ドメインごとに子モジュールへ分割している。
//! - `background`: 背景画像（`BackgroundOverride` / `injected_background_image`）
//! - `brick_image`: ブロック用画像（`BrickImageOverride` / `injected_brick_image`）
//! - `brick_layout`: ブロック配置（`BrickLayout` / `BrickLayoutOverride` /
//!   `default_brick_layout` / `diff_brick_layout` / `injected_brick_layout` /
//!   `injected_cell_size`）
//!
//! これにより「アプリのコード（Rust/WASM）は 1 ビルド」のまま、サービスごとに
//! 背景やブロックの配置・絵柄を React 側から差し替えられる。いずれも Web ビルド専用で、
//! ネイティブビルドでは常にデフォルトへフォールバックする。
//!
//! `window.__BREAKOUT_CONFIG__` の取得（`breakout_config`）と画像バイト列のデコード
//! （`decode_injected_image`）は 3 つの子モジュールが共通で使う処理のため、ここ（親モジュール）
//! に残し、各子モジュールから `super::` 経由で呼ばせている。

mod background;
pub use background::{injected_background_image, BackgroundOverride};

mod brick_image;
pub use brick_image::{injected_brick_image, BrickImageOverride};

mod brick_layout;
pub use brick_layout::{
    default_brick_layout, diff_brick_layout, injected_brick_layout, injected_cell_size,
    BrickLayout, BrickLayoutOverride,
};

// `breakout_config` / `decode_injected_image` はどちらも wasm32 専用なので、
// ネイティブビルドでは未使用警告になる。
#[cfg(target_arch = "wasm32")]
use bevy::prelude::*;

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
