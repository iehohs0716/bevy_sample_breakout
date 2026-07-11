# React 側から Bevy(WASM) に背景画像を渡す設計

日付: 2026-07-11

WASM 化した Bevy Breakout（[[20260711_bevy-wasm-react-integration]] 参照）の背景画像を、
Rust 側にハードコードするのではなく **React 側から与える**構成にした記録。
狙いは「ゲーム本体（WASM）のコードは 1 ビルドのまま、サービスごとに背景だけを差し替える」こと。

---

## 背景・要件

- 当初、背景画像パスは Rust に定数ハードコードされていた
  （`const BACKGROUND_IMAGE_PATH = "backgrounds/background.png"`）。
- 要件は「アプリ(WASM)のコードは 1 つのまま、React 側から背景画像を与え、
  サービスごとに背景だけを差し替えられるようにする」こと。
- 将来的に S3 等の外部 URL の画像を背景に使う可能性がある。

## 設計判断: `bevy_web_asset` は採用せず「バイト列注入」方式

外部 URL を直接ロードする案として `bevy_web_asset`（HTTP アセットソース）を検討したが、
**crates.io を調べた結果、最新の 0.11.0 でも対応 bevy は `^0.16` までで bevy 0.19 に非対応**だった
（0.10.1 = `^0.15`、0.9.0 = `^0.14`）。よって不採用。

代替として採用したのが **バイト列注入方式**:

1. React が背景画像を `fetch` してバイト列（`Uint8Array`）を得る
2. そのバイト列を WASM に渡す
3. Rust が `Image::from_buffer` でデコードして背景スプライトに使う

利点:

- 追加の重い依存が不要。
- 認証や CORS の扱いを React の `fetch` 側に寄せられる。
- CORS 許可が前提なら S3 等の任意 URL にも対応できる。

## 受け渡しの仕組み（JS ↔ Rust）

WASM の `init()` は `main()` を実行するだけで引数を渡せない。そこで
**「起動前に JS グローバルへ載せ、起動時に Rust が読む」**形にした。

```
React: fetch(background) → Uint8Array
     → window.__BREAKOUT_CONFIG__ = { backgroundBytes, backgroundMime }
     → init()  (WASM 起動)
Rust:  main() が起動時に __BREAKOUT_CONFIG__ を読む
     → BackgroundOverride(Some(Image)) を Resource として insert
     → setup システムが取り出して背景スプライトに使う
```

`window.__BREAKOUT_CONFIG__` の形:

```ts
window.__BREAKOUT_CONFIG__ = {
  backgroundBytes: Uint8Array,        // fetch した画像のバイト列
  backgroundMime: string | undefined, // content-type（フォーマット判定用）
};
```

## Rust 実装（`game_engine/src/main.rs`）

React から渡された画像を一時保持する Resource:

```rust
#[derive(Resource, Default)]
struct BackgroundOverride(Option<Image>);
```

WASM 起動時に JS グローバルを読み、`Image` にデコードする関数
（`game_engine/src/main.rs:72`）:

```rust
#[cfg(target_arch = "wasm32")]
fn injected_background_image() -> Option<Image> {
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
        true, // is_srgb
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
```

実装上のポイント:

- `use` は**関数内**で行っている。ネイティブビルドではこの関数が使われないため、
  トップレベルに置くと未使用インポート警告が出るのを避けるため。
- ネイティブ向けには常に `None` を返すスタブを用意し、デフォルト背景を使う:

  ```rust
  #[cfg(not(target_arch = "wasm32"))]
  fn injected_background_image() -> Option<Image> {
      None
  }
  ```

`main()` で Resource として注入する（`game_engine/src/main.rs:125`）:

```rust
App::new()
    .insert_resource(BackgroundOverride(injected_background_image()))
    // ...
```

`setup` システムで取り出す。渡されていればそれを `Assets<Image>` に登録、
無ければ定数パスのデフォルト画像をロードする（`game_engine/src/main.rs:265`）:

```rust
fn setup(
    mut commands: Commands,
    // ...
    mut images: ResMut<Assets<Image>>,
    mut background_override: ResMut<BackgroundOverride>,
    asset_server: Res<AssetServer>,
) {
    // ...
    let background_handle = match background_override.0.take() {
        Some(image) => images.add(image),
        None => asset_server.load(BACKGROUND_IMAGE_PATH),
    };
    commands.spawn((
        Sprite {
            image: background_handle,
            custom_size: Some(BACKGROUND_SIZE),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -10.0), // 最背面
    ));
}
```

## Cargo.toml（`game_engine/Cargo.toml`）

WASM ターゲット専用に JS 連携クレートを追加する。ネイティブビルドには含めない:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
js-sys = "0.3"
web-sys = { version = "0.3", features = ["Window"] }
```

画像フォーマット対応の feature 追加については [[20260711_external-image-cors-and-formats]] を参照。

## React 実装（`frontend/src/components/BevyGame.tsx`, `App.tsx`）

`BevyGame` に `background?: string` prop を追加。相対パスでも外部絶対 URL でも指定できる。
`useEffect` 内で、`init()` の**前に**背景を fetch してグローバルに載せる
（`frontend/src/components/BevyGame.tsx:46`）:

```ts
if (background) {
  try {
    const res = await fetch(background);
    if (!res.ok) {
      throw new Error(`背景画像の取得に失敗: ${res.status} ${res.statusText}`);
    }
    const buf = await res.arrayBuffer();
    const w = window as typeof window & {
      __BREAKOUT_CONFIG__?: { backgroundBytes?: Uint8Array; backgroundMime?: string };
    };
    w.__BREAKOUT_CONFIG__ = {
      backgroundBytes: new Uint8Array(buf),
      backgroundMime: res.headers.get("content-type") ?? undefined,
    };
  } catch (error) {
    console.warn("背景画像の取得に失敗しました。デフォルト背景で起動します。", error);
  }
}
```

fetch 失敗は `try/catch` で握り、`console.warn` してデフォルト背景で起動を継続する
（致命にしない）。この経路が CORS 失敗時のフォールバックとして機能する。

`App.tsx` は URL を渡すだけのデモ:

```tsx
const BACKGROUND_URL = "/assets/backgrounds/sample_sunset.png";
// ...
<BevyGame width={900} height={600} background={BACKGROUND_URL} />
```

### useEffect の依存配列を `[]` にした理由

依存配列は **`[]`** とし、`// eslint-disable-next-line react-hooks/exhaustive-deps` を付けた
（`frontend/src/components/BevyGame.tsx:100`）。

- 背景は**起動時に一度だけ読む値**。Bevy(winit) は再初期化・破棄ができず、
  `startedRef`(useRef) ガードにより effect 本体は一度しか走らない
  （StrictMode の二重実行対策も兼ねる）。
- よって実行中に `background` が変わっても再起動しない＝「起動時の値で固定」という意図。
- 当初 `[background]` にしていたが、exhaustive-deps を満たすだけで実挙動は変わらず、
  「変えたら再描画される」と誤読されうる。意図を明示するため `[]` + disable コメントに変更した。

## 運用上の注意

- WASM 生成物（`public/wasm`・`public/assets`）は gitignore 対象。別チェックアウトや
  本番に反映するには各所で `pnpm build:wasm` の再実行が必要。
- 背景差し替えは「React の `background` prop に渡す URL を変えるだけ」。
  同一 WASM ビルドのまま別背景のサービスを提供できる、が当初要件の達成形。
- 外部画像は必ず CORS 許可済みにするか、同一オリジン配信/プロキシ経由にすること
  （詳細は [[20260711_external-image-cors-and-formats]]）。
