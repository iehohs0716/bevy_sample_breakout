# `?` 演算子と `.ok()?` の組み合わせ（早期リターンの糖衣構文）

日付: 2026-07-31

`breakout_config`（`game_engine/src/injection.rs:19-28`）を題材に、`?` 演算子の意味と、
頻出する `.ok()?` の組み合わせを整理する。

```rust
// game_engine/src/injection.rs:19
fn breakout_config() -> Option<wasm_bindgen::JsValue> {
    use wasm_bindgen::JsValue;

    let window = web_sys::window()?;
    let config = js_sys::Reflect::get(&window, &JsValue::from_str("__BREAKOUT_CONFIG__")).ok()?;
    if config.is_undefined() || config.is_null() {
        return None;
    }
    Some(config)
}
```

前提知識:
- `Reflect::get` / `dyn_into` の失敗を `.ok()?` で丸める理由（本ドキュメントの元ネタ） → [[20260731_reflect-get-and-dyn-into-jsvalue]]
- `Option` を扱う別の定番イディオム（`take` / `unwrap_or_else`） → [[20260729_option-take-unwrap-or-else]]
- `Result` のエラー型変換に関わる `From` トレイト → [[20260725_rust-from-into-trait]]

---

## 1. `?` は何をしているか

`web_sys::window()` は `Option<web_sys::Window>` を返す（ブラウザ上ではまず失敗しないが、
「取得できないかもしれない」という可能性を型で表現している）。

`?` を使わずに書くと、次の `match` と等価になる。

```rust
let window = match web_sys::window() {
    Some(w) => w,
    None => return None,   // breakout_config() 自体をここで終了させる
};
```

つまり `web_sys::window()?` は「成功していれば中身（`Window`）を取り出して次の行に進む。
失敗していれば、それ以上進まずに **`breakout_config` 関数自体を `None` で終了させる**」という
早期リターンの糖衣構文。「無ければ `None` になる」という理解で合っているが、正確には
「無ければ、`?` を書いたその場で関数ごと `None` を返して打ち切る」という動きになる。

## 2. `?` を使うための条件：戻り値の型が一致していること

`?` は「今書いている関数自身の戻り値の型」と「`?` を付けた式の型」が対応していないと使えない。

- `breakout_config` の戻り値は `Option<JsValue>`
- `web_sys::window()` の戻り値は `Option<Window>`

どちらも `Option` なので、失敗時に `None` を返す先として辻褄が合う。もし `breakout_config` が
`Result<JsValue, String>` を返す関数だったら、`Option` を返す式に `?` はそのまま使えない
（`Result` に対する `?` は `Result` を返す関数でしか使えない。逆も同様）。

## 3. `.ok()?`：`Result` を「失敗理由は捨てて」早期リターンする定型パターン

2行目は `?` の直前に `.ok()` が挟まっている。

```rust
js_sys::Reflect::get(&window, &JsValue::from_str("__BREAKOUT_CONFIG__")).ok()?
```

`Reflect::get` の戻り値は `Result<JsValue, JsValue>`（失敗時はJS側の例外情報が `Err` に入る）。
一方 `breakout_config` の戻り値は `Option<JsValue>`。**型が `Result` と `Option` で食い違っている**
ため、`Result` にそのまま `?` は使えない。そこで2段階の変換を挟む。

1. **`.ok()`**：`Result<T, E>` を `Option<T>` に変換する。エラーの中身（`E`）は捨てる。
   ```rust
   Ok(値)   → Some(値)
   Err(何か) → None       // 失敗理由は握りつぶす
   ```
2. **`?`**：その `Option<T>` に対して1節と同じ早期リターンを行う。

```rust
// 上のコードの脱糖（概念）
let result: Result<JsValue, JsValue> = js_sys::Reflect::get(&window, &JsValue::from_str("__BREAKOUT_CONFIG__"));
let opt: Option<JsValue> = result.ok();     // ① 失敗理由を捨てて Option に
let config: JsValue = match opt {           // ② ? の中身
    Some(v) => v,
    None => return None,
};
```

`.ok()?` は「`Result` で返ってくるが、失敗理由には興味がなく、失敗したら即座に関数を諦めたい」
という場面の定型パターン。`breakout_config` は「JSオブジェクトが読めない事情」を呼び出し元に
伝える必要が無く、すべて `None`（＝デフォルトへフォールバック）に丸めたい設計なので、
このパターンが繰り返し現れる（同型のパターンは `injected_brick_image`
（`injection.rs:266-269`）にも登場、詳細は [[20260731_reflect-get-and-dyn-into-jsvalue]]）。

## 4. `?` では拾えない失敗もある

`breakout_config` の最後の `if` は `?` を使っていない。

```rust
if config.is_undefined() || config.is_null() {
    return None;
}
```

`Reflect::get` はJSの `undefined` プロパティを読んでも「取得自体」は成功として扱う
（`Ok(JsValue::undefined())` のように、型としては成功が返ってくる）。つまり
「**型として失敗した**（`Err`/`None`）」場合しか `?` は拾えず、「**型としては成功したが、
中身がJS的に空**」という条件は `?` では表現できない。そのため、ここだけ明示的な
`if` + `return None` で手動チェックしている。

## 5. なぜ例外（try/catch）ではなくこの形なのか

Rust には例外機構が無く、失敗は必ず戻り値の型（`Option` / `Result`）で表現される。`?` は
「その戻り値をいちいち `match` で分解して早期returnする」という頻出パターンを1文字に圧縮した
ものであり、制御フローとしては通常の `return` と同じ（例外のようなスタック巻き戻しの特殊機構
ではない）。

## 6. まとめ

- `expr?` は「`expr` が成功なら中身を取り出して続行、失敗なら **今の関数自体を** 同じ種類の
  失敗（`None`/`Err`）で即終了させる」の糖衣構文。`match` 1回分を1文字に圧縮したもの。
- 使うには、`?` を書いている関数の戻り値の型が `expr` の型（`Option`/`Result`）と一致している
  必要がある。
- `.ok()?` は「`Result` だが失敗理由はどうでもよく、`Option` として早期リターンしたい」ときの
  定型2段コンボ：`.ok()` で `Result→Option`（エラー内容を破棄）、`?` で早期リターン。
- 「型として失敗（`None`/`Err`）」と「型としては成功だが値の中身がJS的に空」は別種の失敗なので、
  後者は `?` では拾えず明示的な `if` + `return` が必要（`is_undefined()` 判定）。
- `web_sys::window()?` のケースで言えば「window が取れなければ `breakout_config` 全体が
  即座に `None` になる」という理解で正しい。
