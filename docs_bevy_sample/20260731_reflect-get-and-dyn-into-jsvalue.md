# `js_sys::Reflect::get` と `JsCast::dyn_into`（JS↔Rust 境界の値の取り出し方）

日付: 2026-07-31

`injected_brick_image`（`game_engine/src/injection.rs:266-270`）の以下のコードを題材に、
WASM 側から `window.__BREAKOUT_CONFIG__` のようなJSオブジェクトのプロパティを読み出す際の
定型パターンを整理する。

```rust
// game_engine/src/injection.rs:266
let bytes = js_sys::Reflect::get(&entry, &JsValue::from_str("bytes"))
    .ok()?
    .dyn_into::<js_sys::Uint8Array>()
    .ok()?
    .to_vec();
```

同型のコードは `injected_background_image`（`injection.rs:82-83`）にも重複して現れる。

前提知識:
- JS からの初期化パラメータ受け渡し全体の設計 → [[20260711_react-to-bevy-init-params]]
- 同じパターンの初出（背景画像バイト列注入） → [[20260711_react-to-bevy-background-injection]]
- `.into()`（型変換トレイト）との混同に注意 → [[20260725_rust-from-into-trait]]
- `Option::take` / `?` による早期リターンの読み方 → [[20260729_option-take-unwrap-or-else]]

---

## 1. なぜ `entry.bytes` と書けないのか

`entry` は Rust の型システムから見ると `JsValue` ——「JSの世界の値を包んだ、中身不明の箱」でしか
ない。Rust 構造体のようにフィールド名でアクセスする構文（`entry.bytes`）は使えない。

JS側で見ればただの `entry.bytes` だが、Rust 側では「名前を指定してプロパティを取得する」という
**関数呼び出し**の形を取る必要がある。それが `js_sys::Reflect::get`。

```rust
js_sys::Reflect::get(&entry, &JsValue::from_str("bytes"))
```

これは JS の [`Reflect.get(entry, "bytes")`](https://developer.mozilla.org/ja/docs/Web/JavaScript/Reference/Global_Objects/Reflect/get) をそのままRustから呼んでいるだけで、`entry["bytes"]` と等価。
プロパティ名を毎回 `JsValue::from_str(...)` で包むのは、JSの世界では文字列もまた「値」であり、
`Reflect::get` の第2引数が `&JsValue` を要求するため。

戻り値は `Result<JsValue, JsValue>`。取得自体は成功しても、中身が `Uint8Array` なのか
`string` なのか `undefined` なのかは、この時点ではまだ**型が付いていない**。

## 2. `dyn_into::<T>()` は実行時の型チェック付きダウンキャスト

`Reflect::get` が返す `JsValue` を、次の行で具体的な型に変換しているのが

```rust
.dyn_into::<js_sys::Uint8Array>()
```

`dyn_into` は `wasm_bindgen::JsCast` トレイトのメソッド（`injection.rs:257` の
`use wasm_bindgen::{JsCast, JsValue};` で導入している）。シグネチャの概念は次の通り:

```rust
// wasm-bindgen（概念）
trait JsCast {
    fn dyn_into<T: JsCast>(self) -> Result<T, JsValue>;
}
```

やっていることは「この `JsValue`、実行時に調べて本当に `Uint8Array` だったら `Uint8Array` 型に
包み直す。違ったら失敗として元の `JsValue` を返す」という**実行時型チェック**。

- 成功: `Ok(Uint8Array)` — 以後 `Uint8Array` 専用のメソッド（`to_vec()` 等）が使える。
- 失敗: `Err(元のJsValue)` — 値自体は失われず、失敗した箱がそのまま返る。

Rust本来の `Box<dyn Any>` に対する `downcast::<T>()` に近い。コンパイル時には
「型が合っているか」を保証できない（JSは動的型付けなので当然）ため、境界を跨ぐ場所でのみ
この実行時チェックが必要になる。似た役割の `dyn_ref::<T>()`（所有権を取らず `Option<&T>` を返す
版）もあるが、ここでは直後に `to_vec()` で消費するだけなので所有権ごと取る `dyn_into` を使っている。

## 3. `.ok()?` の連鎖が意味すること

このコードの `Result` / `Option` の連鎖は、**JSから渡ってくる値には何の保証もない**という前提に
基づいている。

```rust
js_sys::Reflect::get(&entry, &JsValue::from_str("bytes"))  // Result<JsValue, JsValue>
    .ok()?                                                  // Option<JsValue> にして早期return
    .dyn_into::<js_sys::Uint8Array>()                       // Result<Uint8Array, JsValue>
    .ok()?                                                  // Option<Uint8Array> にして早期return
    .to_vec();                                               // Vec<u8>
```

| 失敗しうるケース | どのステップで弾かれるか |
|---|---|
| `entry` に `bytes` プロパティ自体が無い（プロパティ取得は成功し `undefined` が返る） | `dyn_into` が `Uint8Array` への変換に失敗 |
| `bytes` はあるが文字列や数値など別の型だった | 同上 |
| `Reflect::get` 自体が失敗する（`entry` が null/undefined でプロパティアクセス不能等） | 1つ目の `.ok()?` |

`.ok()` はエラー内容（`JsValue`）を握りつぶして `Option` に変換している。呼び出し元の
`injected_brick_image` 自体が `Option<Image>` を返す設計（`injection.rs:256`）なので、
「JSの都合で起きうる失敗はすべて `None` に丸めてデフォルト（単色ブロック）にフォールバックする」
という一貫した方針に沿っている。`Result` のままエラー内容を伝播させる必要が無いため、
`.ok()?` で握りつぶす選択は妥当。

## 4. `.into()`（型変換トレイト）と混同しないこと

`dyn_into` は名前に `into` を含むが、[[20260725_rust-from-into-trait]] で扱った
`Into` トレイトの `.into()` とは**無関係の別トレイト**（`JsCast::dyn_into`）。

- `.into()`（`Into`）: コンパイル時に確定した型変換。失敗しない（`Result` を返さない）。
- `.dyn_into::<T>()`（`JsCast`）: 実行時の型チェックを伴うダウンキャスト。失敗しうる
  （`Result<T, JsValue>` を返す）。

「JSとの境界を越える変換は失敗しうる」という一点が、両者を区別する最大の判断基準になる。

## 5. まとめ

- `JsValue` は「型情報が消えたJSの値」。フィールドアクセス構文が使えないため、
  プロパティ取得には `js_sys::Reflect::get(&obj, &JsValue::from_str("prop"))` という
  関数呼び出しの形を取る（`entry["prop"]` 相当）。
- `dyn_into::<T>()`（`JsCast` トレイト）は「実行時にチェックしてから具体的な型 `T` に
  変換し直す」ダウンキャスト。失敗時は元の `JsValue` を保ったまま `Err` になる。
- `.into()`（`Into` トレイト）とは名前が似ているだけの別物。前者はコンパイル時変換で失敗しない、
  後者は実行時変換で失敗しうる、という違いで区別する。
- `.ok()?` の連鎖は「JSから来る値は何も保証されていない」という前提のもと、あらゆる失敗を
  `None` に丸めてフォールバック値（単色ブロック／デフォルト背景）を使わせるための定型パターン。
