# React prop から Bevy(WASM) へ初期化パラメータを渡す仕組み（背景画像・初期ブロック配置）

日付: 2026-07-11

React 側の prop で指定した値（背景画像・初期ブロック配置）を、WASM 化した Bevy Breakout の
**起動時**に注入する仕組みの記録。背景に限らず「React から Bevy へ初期化パラメータを渡す」
汎用パターンとして整理する。背景画像のみの初期版の経緯は
[[20260711_react-to-bevy-background-injection]]、外部 URL の CORS / 画像フォーマットは
[[20260711_external-image-cors-and-formats]]、WASM 化の全体構成は
[[20260711_bevy-wasm-react-integration]] を参照。なお、この `web_sys`/`js_sys` 越しの
window 読み取りが「capability ベース」というセキュリティモデルの実例になっている点は
[[20260711_why-wasm-is-secure]] を参照。

---

## 1. 最重要ポイント（誤解しやすい核心）

- **「React が Rust の変数を直接書き換える」わけではない。** 実際は次のバケツリレー:
  1. JS が `window` のグローバル変数（`window.__BREAKOUT_CONFIG__`）を *郵便受け* にして値を投函する。
  2. Rust は起動時（`main()`）に **一度だけ** その郵便受けを読み取り、自分の `Resource` に写し取る。
  3. `setup` システムがその `Resource` を消費してエンティティを spawn する。
- したがって **起動時に一度だけ有効**。`window.__BREAKOUT_CONFIG__` を後から書き換えても
  Rust は再読しない。実行中の動的変更が必要なら、別の仕組み（`#[wasm_bindgen]` で Rust 関数を
  公開して `World` を操作する等）が要る。
- `init()`（WASM 起動）は引数を取れず `main()` を実行するだけなので、この「起動前に投函」形式を採る。
  よって **必ず `init()` より前に投函する**（順序が逆だと Rust が読む時点で郵便受けが空）。

## 2. 全体のデータフロー

```
App.tsx (prop で背景URL・ブロック配置を指定)
  ↓
BevyGame.tsx (config オブジェクトを組み立て)
  ↓  背景は fetch してバイト列化
window.__BREAKOUT_CONFIG__ = config      ← 郵便受けに投函（init より前）
  ↓
init()  (WASM 起動 = main() 実行)
  ↓
main.rs: injected_background_image() / injected_brick_layout()  ← 郵便受けを読み Rust 型へ変換
  ↓
.insert_resource(BackgroundOverride(...)) / BrickLayoutOverride(...)  ← Resource に格納
  ↓
setup(): Resource を take() → あれば注入値で spawn / 無ければデフォルトにフォールバック
```

## 3. JS ↔ Rust の契約

`window.__BREAKOUT_CONFIG__` の形（両側でキー名と形を一致させる必要がある）:

```ts
window.__BREAKOUT_CONFIG__ = {
  backgroundBytes?: Uint8Array,          // fetch 済み背景画像のバイト列
  backgroundMime?: string,               // content-type（フォーマット判定用）
  bricks?: Array<{ x: number; y: number }>, // 各ブロックの中心座標
  cellSize?: { width: number; height: number }, // 全ブロック共通のセルサイズ
};
```

### 座標系（重要）

`bricks` の `x, y` は **Bevy のワールド座標**であって DOM のピクセル座標ではない:

- 中心原点・y 上向き・1 単位 = 1px。
- アリーナは `x ∈ [-450, 450]`, `y ∈ [-300, 300]`（`main.rs:26-30` の `LEFT/RIGHT/BOTTOM/TOP_WALL`）。
- 各要素の値はブロックの **中心** 座標（Bevy の `translation` は中心を表す）。

### フォールバック規約

| 条件 | 挙動 |
|------|------|
| `backgroundBytes` 未指定 / 空 / デコード失敗 | Rust 側デフォルト背景 `backgrounds/background.png` |
| `bricks` 未指定 / 空 / x,y 欠落 | Rust 側デフォルト敷き詰め配置 |
| `bricks` 指定あり + `cellSize` あり | その `cellSize` を全ブロックに適用 |
| `bricks` 指定あり + `cellSize` 無 / 不正 | Rust 側固定 `BRICK_SIZE` (50x30, `main.rs:32`) |
| `bricks` 無で `cellSize` のみ | `cellSize` は無視（デフォルト敷き詰めは常に固定 50x30 を使う） |

## 4. JS 側（投函）: `frontend/src/components/BevyGame.tsx`

`useEffect` 内で `config` を組み立て、背景を fetch してから `window` に載せ、その **後** に
`init()` を呼ぶ（`frontend/src/components/BevyGame.tsx:67-105`）:

```ts
const config: {
  backgroundBytes?: Uint8Array; backgroundMime?: string;
  bricks?: Array<{ x: number; y: number }>; cellSize?: { width: number; height: number };
} = {};

if (background) {
  try {
    const res = await fetch(background);
    if (!res.ok) throw new Error(`背景画像の取得に失敗: ${res.status} ${res.statusText}`);
    const buf = await res.arrayBuffer();
    config.backgroundBytes = new Uint8Array(buf);
    config.backgroundMime = res.headers.get("content-type") ?? undefined;
  } catch (error) {
    console.warn("背景画像の取得に失敗しました。デフォルト背景で起動します。", error);
  }
}

if (bricks && bricks.length > 0) {
  config.bricks = bricks;
  if (cellSize) config.cellSize = cellSize;
}

const w = window as typeof window & { __BREAKOUT_CONFIG__?: typeof config };
w.__BREAKOUT_CONFIG__ = config;   // ← 投函（この後で init() を呼ぶ）
```

`App.tsx` は prop を渡すだけのデモ。背景 URL とピラミッド型のブロック配置を生成して渡す
（`frontend/src/App.tsx:14-48`）。ブロック配置は「各段が下ほど増えるピラミッド」を
ワールド座標で組み立てて `bricks` に渡している。

## 5. Rust 側（読取・変換）: `game_engine/src/main.rs`

背景・ブロックとも **同じパターン**。WASM 専用関数が `web_sys` / `js_sys` の
`Reflect::get` で郵便受けを引き、`dyn_into` / `as_f64` で JS 値を Rust 型に詰め替える。
ネイティブビルドでは常に `None` を返すスタブに `#[cfg]` で切り替わる。

ブロック配置の読取（`game_engine/src/main.rs:142-198`）:

```rust
#[cfg(target_arch = "wasm32")]
fn injected_brick_layout() -> Option<BrickLayout> {
    use wasm_bindgen::{JsCast, JsValue};
    let window = web_sys::window()?;
    let config = js_sys::Reflect::get(&window, &JsValue::from_str("__BREAKOUT_CONFIG__")).ok()?;
    if config.is_undefined() || config.is_null() { return None; }

    let bricks_arr = js_sys::Reflect::get(&config, &JsValue::from_str("bricks"))
        .ok()?.dyn_into::<js_sys::Array>().ok()?;
    if bricks_arr.length() == 0 { return None; }

    let mut positions = Vec::with_capacity(bricks_arr.length() as usize);
    for i in 0..bricks_arr.length() {
        let brick = bricks_arr.get(i);
        let x = js_sys::Reflect::get(&brick, &JsValue::from_str("x")).ok().and_then(|v| v.as_f64());
        let y = js_sys::Reflect::get(&brick, &JsValue::from_str("y")).ok().and_then(|v| v.as_f64());
        match (x, y) {
            (Some(x), Some(y)) => positions.push(Vec2::new(x as f32, y as f32)),
            _ => warn!("ブロック配置の要素 {i} に数値の x/y が無いためスキップします"),
        }
    }
    if positions.is_empty() { return None; }
    // cellSize は無ければ BRICK_SIZE にフォールバック（幅・高さが正のときだけ採用）
    // ... unwrap_or(BRICK_SIZE)
    Some(BrickLayout { positions, cell_size })
}

#[cfg(not(target_arch = "wasm32"))]
fn injected_brick_layout() -> Option<BrickLayout> { None }
```

背景画像も同型で、読み取ったバイト列を `Image::from_buffer`（MIME → `ImageType`、失敗時 `None`）
でデコードする（`game_engine/src/main.rs:72-115`）。詳細は
[[20260711_react-to-bevy-background-injection]]。

一時保持する `Resource` と受け皿の構造体（`main.rs:66-67`, `127-136`）:

```rust
#[derive(Resource, Default)]
struct BackgroundOverride(Option<Image>);

struct BrickLayout { positions: Vec<Vec2>, cell_size: Vec2 }

#[derive(Resource, Default)]
struct BrickLayoutOverride(Option<BrickLayout>);
```

## 6. Resource への格納: `main()`（`game_engine/src/main.rs:224-227`）

```rust
App::new()
    .insert_resource(BackgroundOverride(injected_background_image()))
    .insert_resource(BrickLayoutOverride(injected_brick_layout()))
    // ...
```

`injected_*()` の戻り値をそのまま `Resource` として差し込む。WASM では郵便受けの内容、
ネイティブでは常に `None`。

## 7. 消費: `setup` システム（`game_engine/src/main.rs:351-485`）

`Resource` を `take()` して、あれば注入配置で spawn、無ければデフォルトにフォールバックする。
spawn ロジックは `spawn_brick` ヘルパー（`main.rs:208-222`）に一本化し、デフォルト配置と
注入配置の双方から使う。

```rust
// 背景（main.rs:368-379）
let background_handle = match background_override.0.take() {
    Some(image) => images.add(image),                 // 注入画像を登録
    None => asset_server.load(BACKGROUND_IMAGE_PATH),  // デフォルト
};

// ブロック（main.rs:443-448）
if let Some(layout) = brick_layout_override.0.take() {
    for position in &layout.positions {
        spawn_brick(&mut commands, *position, layout.cell_size);
    }
    return; // 注入があればここで終了
}
// 以降はデフォルトの敷き詰め配置（固定 BRICK_SIZE で行列を計算）
```

## 8. 依存: `game_engine/Cargo.toml`

JS 連携クレートは **WASM ターゲット専用**で追加し、ネイティブビルドには含めない
（`game_engine/Cargo.toml:14-17`）:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
js-sys = "0.3"
web-sys = { version = "0.3", features = ["Window"] }
```

`#[cfg(target_arch = "wasm32")]` 分岐と対で、JS 依存を wasm32 に閉じている。ネイティブ側は
`injected_*()` が常に `None` を返すスタブなので、これらのクレートを一切参照しない
（余計な依存とビルドを避けられる）。背景の jpeg / webp 対応 feature は `Cargo.toml:9`。

## 9. この設計の狙い

WASM 生成物（約 57MB）を 1 ビルドに保ったまま、React 側のパラメータ差し替えだけで
サービスごとに別背景・別ブロック配置を提供できる。ゲーム本体を焼き直す必要がない。

## 10. 実務上の注意（検証で判明）

- **外部 URL 背景は CORS が必要**。例として `images.ygoprodeck.com` の画像を試したが、
  localhost からの fetch は配信元が CORS を許可しておらず拒否され、デフォルト背景に
  フォールバックした（`try/catch` で握り致命化しない）。詳細は
  [[20260711_external-image-cors-and-formats]]。
- **StrictMode 二重 effect 対策**として `useRef` ガードで `init` を一度だけ呼ぶ
  （`BevyGame.tsx:53-61`）。`useEffect` の依存配列は意図的に `[]`（起動時固定の値のため）。
- **実ブラウザ検証済み**（Playwright headless + SwiftShader）。「ビルド成功＝表示成功」では
  ないため、描画は必ず実ブラウザで確認する。手法は [[20260711_wasm-bevy-browser-verification]]。
