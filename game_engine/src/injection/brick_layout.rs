//! React（JS）から渡された「初期ブロック配置」の読み取り。
//! `setup` からのみ使う。

use bevy::prelude::*;

// ネイティブビルドでは `injected_brick_layout`（wasm32専用）内でしか使わないため未使用警告になる。
#[cfg(target_arch = "wasm32")]
use crate::{components::BrickCell, config::BRICK_SIZE};

use crate::common::brick::layout::BrickLayout;

/// Web ビルド専用。`window.__BREAKOUT_CONFIG__.cellSize`（`{width, height}`）を読む共通処理。
/// `injected_brick_layout()`（`bricks` とセットの場合）と `injected_cell_size()`（`bricks` が
/// 無くても、画像差分の自動配置のセルサイズとして使う場合）の両方から呼ばれる。
#[cfg(target_arch = "wasm32")]
fn read_cell_size(config: &wasm_bindgen::JsValue) -> Option<Vec2> {
    use wasm_bindgen::JsValue;

    js_sys::Reflect::get(config, &JsValue::from_str("cellSize"))
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
}

/// Web ビルド専用。`window.__BREAKOUT_CONFIG__.cellSize` を、`bricks`（明示配置）の有無に
/// 関わらず読む。`bricks` が無い場合の 2 経路（`diff_brick_layout` / `default_brick_layout`）
/// のセルサイズとして使うためのもの。指定が無い / 不正な場合は `None`
/// （呼び出し側で `BRICK_SIZE` にフォールバックする）。
#[cfg(target_arch = "wasm32")]
pub fn injected_cell_size() -> Option<Vec2> {
    let config = super::breakout_config()?;
    read_cell_size(&config)
}

/// ネイティブビルドでは JS からの注入は無い（常にデフォルトの `BRICK_SIZE` を使う）。
#[cfg(not(target_arch = "wasm32"))]
pub fn injected_cell_size() -> Option<Vec2> {
    None
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

    let config = super::breakout_config()?;

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
    let cell_size = read_cell_size(&config).unwrap_or(BRICK_SIZE);

    // JS 側の座標は必ずしも 0 始まりではないので、最小値を格子の原点として行列座標を逆算する
    // (格子に整合していることは前提とし、四則演算のみで求める)。
    let origin_x = positions
        .iter()
        .map(|p| p.x)
        .fold(f32::INFINITY, f32::min);
    let origin_y = positions
        .iter()
        .map(|p| p.y)
        .fold(f32::INFINITY, f32::min);
    let cells = positions
        .iter()
        .map(|p| BrickCell {
            row: ((p.y - origin_y) / cell_size.y).round() as i32,
            col: ((p.x - origin_x) / cell_size.x).round() as i32,
        })
        .collect();

    Some(BrickLayout {
        positions,
        cell_size,
        cells,
    })
}

/// ネイティブビルドでは JS からの注入は無い（常にデフォルト配置を使う）。
#[cfg(not(target_arch = "wasm32"))]
pub fn injected_brick_layout() -> Option<BrickLayout> {
    None
}
