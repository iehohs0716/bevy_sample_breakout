//! React（JS）から `window.__BREAKOUT_CONFIG__` 経由で渡された初期化パラメータ
//! （背景画像・初期ブロック配置・ブロック用画像）の読み取りと、それを一時保持する Resource。
//!
//! これにより「アプリのコード（Rust/WASM）は 1 ビルド」のまま、サービスごとに
//! 背景やブロックの配置・絵柄を React 側から差し替えられる。いずれも Web ビルド専用で、
//! ネイティブビルドでは常にデフォルトへフォールバックする。

use bevy::prelude::*;

use crate::components::BrickCell;
use crate::config::{
    BOTTOM_WALL, BRICK_DIFF_COLOR_THRESHOLD, BRICK_DIFF_LAYOUT_MIN_HEIGHT_RATIO, BRICK_SIZE,
    GAP_BETWEEN_BRICKS, GAP_BETWEEN_BRICKS_AND_CEILING, GAP_BETWEEN_BRICKS_AND_SIDES,
    GAP_BETWEEN_PADDLE_AND_BRICKS, LEFT_WALL, RIGHT_WALL, TOP_WALL,
};
use crate::util::{average_color, inscribed_source_rect};

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
    pub cells: Vec<BrickCell>,
}

/// アリーナを敷き詰めるブロックグリッドの全候補セル（`position`, `BrickCell`）を計算する。
/// `default_brick_layout`（無条件に全セルを採用）と `diff_brick_layout`（一部だけを間引く）の
/// 両方が同じグリッド形状を前提にするための共有ロジック。`paddle_y` はパドルの中心 y で、
/// ブロック帯はその上に `GAP_BETWEEN_PADDLE_AND_BRICKS` だけ空けて始まる。`cell_size` は
/// 1 セルの大きさ（JS が `cellSize` を指定していればそれ、無ければ `BRICK_SIZE`。呼び出し側が
/// 解決する）。
fn brick_grid_candidates(paddle_y: f32, cell_size: Vec2) -> (Vec<Vec2>, Vec<BrickCell>) {
    let total_width_of_bricks = (RIGHT_WALL - LEFT_WALL) - 2. * GAP_BETWEEN_BRICKS_AND_SIDES;
    let bottom_edge_of_bricks = paddle_y + GAP_BETWEEN_PADDLE_AND_BRICKS;
    let total_height_of_bricks = TOP_WALL - bottom_edge_of_bricks - GAP_BETWEEN_BRICKS_AND_CEILING;

    assert!(total_width_of_bricks > 0.0);
    assert!(total_height_of_bricks > 0.0);

    // 使える面積に何行・何列入るか（切り捨て）。
    let n_columns = (total_width_of_bricks / (cell_size.x + GAP_BETWEEN_BRICKS)).floor() as usize;
    let n_rows = (total_height_of_bricks / (cell_size.y + GAP_BETWEEN_BRICKS)).floor() as usize;
    let n_vertical_gaps = n_columns - 1;

    // 列数を丸めたぶん帯の幅は領域より狭くなるので、中心から帯の半分だけ戻して中央揃えにする。
    let center_of_bricks = (LEFT_WALL + RIGHT_WALL) / 2.0;
    let left_edge_of_bricks = center_of_bricks
        - (n_columns as f32 / 2.0 * cell_size.x)
        - n_vertical_gaps as f32 / 2.0 * GAP_BETWEEN_BRICKS;

    // `translation` は中心座標なので、左下の縁から半セルぶん内側を最初のブロックの中心にする。
    let offset_x = left_edge_of_bricks + cell_size.x / 2.;
    let offset_y = bottom_edge_of_bricks + cell_size.y / 2.;

    let mut positions = Vec::with_capacity(n_rows * n_columns);
    let mut cells = Vec::with_capacity(n_rows * n_columns);
    for row in 0..n_rows {
        for column in 0..n_columns {
            positions.push(Vec2::new(
                offset_x + column as f32 * (cell_size.x + GAP_BETWEEN_BRICKS),
                offset_y + row as f32 * (cell_size.y + GAP_BETWEEN_BRICKS),
            ));
            cells.push(BrickCell {
                row: row as i32,
                col: column as i32,
            });
        }
    }

    (positions, cells)
}

/// React（JS）由来の配置が無いときに使う、アリーナを敷き詰めるデフォルトのブロック配置を計算して返す。
/// `injected_brick_layout()`（外部指定）と対になる「組み込みの標準配置」。`cell_size` は
/// 呼び出し側（`setup`）が `injected_cell_size().unwrap_or(BRICK_SIZE)` で解決したものを渡す。
pub fn default_brick_layout(paddle_y: f32, cell_size: Vec2) -> BrickLayout {
    let (positions, cells) = brick_grid_candidates(paddle_y, cell_size);
    BrickLayout {
        positions,
        cell_size,
        cells,
    }
}

/// React（JS）が背景画像・ブロック画像の両方を注入し、かつブロック配置を明示指定していない
/// ときに使う、2 画像の差分から自動でブロック配置を決める処理。`background` / `brick_image` は
/// どちらもアリーナに contain フィットで貼ったと仮定し（`common::brick::texture_crop` の
/// テクスチャ切り出しと同じ写像）、候補グリッド（`brick_grid_candidates`）の各セルについて
/// 対応する領域の平均色を比較する。RGB のユークリッド距離が `BRICK_DIFF_COLOR_THRESHOLD`
/// を超えたセルだけをブロックとして採用し、さらにバー（パドル）からアリーナ天井までの高さの
/// `BRICK_DIFF_LAYOUT_MIN_HEIGHT_RATIO` 未満のセルは（差分があっても）対象外にする
/// （どちらかの画像で内接矩形の外に出るセルは、そちら側を黒として比較する）。`cell_size` は
/// `default_brick_layout` と同様、呼び出し側が解決したものを渡す（差分検出の粒度もこれに従う）。
/// 1 つも採用が無ければ `None`（呼び出し側で `default_brick_layout` にフォールバックする）。
pub fn diff_brick_layout(
    background_image: &Image,
    brick_image: &Image,
    paddle_y: f32,
    cell_size: Vec2,
) -> Option<BrickLayout> {
    let field = Vec2::new(RIGHT_WALL - LEFT_WALL, TOP_WALL - BOTTOM_WALL);
    let background_image_size = Vec2::new(background_image.width() as f32, background_image.height() as f32);
    let brick_image_size = Vec2::new(brick_image.width() as f32, brick_image.height() as f32);

    // とりあえず一通り、全てを網羅するようブロックを作成しておく
    let (candidate_positions, candidate_cells) = brick_grid_candidates(paddle_y, cell_size);

    let mut positions = Vec::new();
    let mut cells = Vec::new();
    for (position, cell) in candidate_positions.into_iter().zip(candidate_cells) {
        let height_ratio = (position.y - paddle_y) / (TOP_WALL - paddle_y);
        if height_ratio < BRICK_DIFF_LAYOUT_MIN_HEIGHT_RATIO {
            continue;
        }

        let background_color = match inscribed_source_rect(position, cell_size, field, background_image_size) {
            Some(rect) => average_color(background_image, rect),
            // 内接矩形の外＝黒扱い（`brick_image_rect` の黒塗りフォールバックと一貫させる）。
            None => Vec4::ZERO,
        };
        let brick_color = match inscribed_source_rect(position, cell_size, field, brick_image_size) {
            Some(rect) => average_color(brick_image, rect),
            None => Vec4::ZERO,
        };

        // アルファは比較に含めず RGB のみで差分を見る（透過対応は必要になった時点で拡張する）。
        let diff = (background_color.truncate() - brick_color.truncate()).length();
        if diff > BRICK_DIFF_COLOR_THRESHOLD {
            positions.push(position);
            cells.push(cell);
        }
    }

    if positions.is_empty() {
        return None;
    }

    Some(BrickLayout {
        positions,
        cell_size,
        cells,
    })
}

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
    let config = breakout_config()?;
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
