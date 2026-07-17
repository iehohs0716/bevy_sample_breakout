# `window.dispatch_event(&event)` の仕組み — WASM から DOM イベントを発火する

日付: 2026-07-17

Bevy(Rust/WASM) からフロント(JS) へ通知するのに使った
`window.dispatch_event(&event)`（＝JS の `window.dispatchEvent(new CustomEvent(...))`）が、
**なぜ Rust から呼べて、React の `addEventListener` に届くのか**を、DOM イベントモデルと
wasm-bindgen/web-sys の橋渡しの両面から解説する。

- この API を使った実装: [[20260717_game-state-and-clear-notification-impl]]
- 設計方針（なぜ通知方式なのか）: [[20260717_bevy-to-frontend-event-notification]]
- WASM が「なぜ JS/DOM を呼べるのか」の土台: [[20260711_bevy-wasm-react-integration]] / [[20260711_why-wasm-is-secure]]

---

## 前提: WASM は DOM を直接触れない。JS 経由で触る

WebAssembly 自体には DOM も `window` も無い。線形メモリと数値演算しか持たない。
ブラウザの API（DOM 含む）は**すべて JS の関数を経由**して呼ぶ。その橋渡しを自動生成するのが
**wasm-bindgen**、生成済みの DOM/Web API バインディングが **web-sys** である。

つまり Rust の `window.dispatch_event(&event)` は、最終的に JS の
`EventTarget.prototype.dispatchEvent.call(window, event)` を呼んでいるにすぎない。

---

## パート1: DOM イベントモデル（JS 側で何が起きるか）

### 登場人物
- **`EventTarget`**: イベントを受け取れるオブジェクトのインターフェース。`window` / `document` /
  各 DOM 要素が実装する。`addEventListener` / `removeEventListener` / `dispatchEvent` を持つ。
- **`Event` / `CustomEvent`**: イベントオブジェクト。`CustomEvent` は任意データを載せる
  **`detail`** フィールドを持つ拡張。`new CustomEvent(type, { detail })` で作る。
- **`type`（イベント名）**: 任意の文字列。今回は衝突回避のため `"breakout:gameclear"` と名前空間を付けた。

### 発火 → 受信の流れ
```js
// 送信側（Bevy 相当）
const ev = new CustomEvent("breakout:gameclear", { detail: { score: 42 } });
window.dispatchEvent(ev);

// 受信側（React）
window.addEventListener("breakout:gameclear", (e) => {
  console.log(e.detail.score); // 42
});
```

`dispatchEvent` は、その `EventTarget`（ここでは `window`）に登録された同名リスナを探して呼ぶ。
リスナには発火時に渡された `Event` オブジェクトが渡り、`e.detail` で `{ score: 42 }` を読める。

### 重要な性質
- **同期実行**: `dispatchEvent` は**その場でリスナを同期的に全部呼び終えてから戻る**。
  キューには積まれない。だから Rust 側で `window.dispatch_event(&event)` を呼んだ瞬間、
  同じコールスタック上で React のハンドラ（`location.reload()` 等）まで走る。
  WASM はシングルスレッドなので、この同期呼び出しは競合なく安全。
- **バブリング**: `CustomEvent` は既定で `bubbles: false`。ただし今回は `window` に対して
  直接 dispatch し、`window` で listen しているので、バブリングの有無は関係なく届く。
- **戻り値**: `dispatchEvent` はイベントが `cancelable` かつリスナが `preventDefault()` を
  呼ぶと `false` を返す。今回は使わないので無視（Rust 側で `let _ =`）。
- **リスナ未登録なら黙って何も起きない**: 受信側が居なくてもエラーにならない。だから React が
  まだ `addEventListener` していないと通知は取りこぼす（購読は WASM 起動前後で確実に張っておく）。

---

## パート2: web-sys / wasm-bindgen の橋渡し（Rust 側がどう JS になるか）

### web-sys のオブジェクトは「JS オブジェクトのハンドル」
`web_sys::CustomEvent` や `web_sys::Window` は、中身に**本物の JS オブジェクトへの参照
（`JsValue`）**を持つ薄いラッパ。Rust のメソッド呼び出しが、対応する JS の操作に 1:1 で対応する。

| Rust（web-sys） | 実際の JS |
|---|---|
| `web_sys::window()` | `globalThis.window` を取得 |
| `js_sys::Object::new()` | `new Object()`（＝ `{}`） |
| `js_sys::Reflect::set(&o, &"score".into(), &v)` | `Reflect.set(o, "score", v)` → `o.score = v` |
| `JsValue::from_f64(42.0)` | JS の数値 `42` |
| `CustomEventInit::new()` + `.set_detail(&o)` | 辞書 `{ detail: o }` を組み立てる |
| `CustomEvent::new_with_event_init_dict("x", &init)` | `new CustomEvent("x", init)` |
| `window.dispatch_event(&event)` | `window.dispatchEvent(event)` |

`new_with_event_init_dict` のような長い名前は、WebIDL のオーバーロード（引数違い）を
Rust の別名関数に展開した結果。`dispatch_event` が `Window` から呼べるのは、`web_sys::Window` が
`Deref` で `EventTarget` に辿れる（JS のプロトタイプ継承 `Window : EventTarget` を反映）ため。

### wasm-bindgen が生成する「グルー JS」
ビルド時に wasm-bindgen CLI が `.wasm` を解析し、**インポート関数を実装する JS グルー**
（本プロジェクトでは `frontend/public/wasm/breakout.js`。[[20260711_bevy-wasm-react-integration]]）を生成する。
呼び出しの実体は概ねこうなる:

```
[Rust] window.dispatch_event(&event)
   ↓ コンパイル: wasm の import 関数 __wbg_dispatchEvent_xxx(window_ptr, event_ptr) を呼ぶ
[glue JS] __wbg_dispatchEvent_xxx = (w, e) =>
             getObject(w).dispatchEvent(getObject(e));   // ハンドル→実オブジェクトに解決して呼ぶ
   ↓
[ブラウザ] 本物の window.dispatchEvent が走り、リスナを同期実行
```

- `getObject(...)` は、wasm 側が持つ整数ハンドルを、グルーが管理する JS オブジェクト表
  （ヒープ）上の**本物のオブジェクト**に引き直す仕組み。Rust の `&CustomEvent` は、この表への
  インデックスとしてグルーに渡っている。
- `detail` に載せた `js_sys::Object`（＝ JS の `{}`）も同じ表の実オブジェクトなので、
  受信側 JS では `e.detail.score` としてそのまま読める（コピーではなく同一オブジェクト）。

### データの流れ（score = 42 の例）
```
Rust usize 42
  → JsValue::from_f64(42.0)      // JS number 42
  → Reflect::set(detail,"score") // detail = { score: 42 }（JS オブジェクト）
  → CustomEventInit.detail       // { detail: { score: 42 } }
  → new CustomEvent(name, init)  // CustomEvent。event.detail === detail
  → window.dispatchEvent(event)  // 同期でリスナ実行
React: e.detail.score            // 42
```

---

## まとめ / 落とし穴
- `window.dispatch_event(&event)` は「Rust の関数呼び出し」に見えて、実体は
  **wasm → グルー JS → 本物の `dispatchEvent`** という 3 段の橋渡し。web-sys がその型付けを、
  wasm-bindgen がその配線を担う。
- **同期実行**なので、通知した瞬間に受信側ハンドラまで走り切る。重い処理や再入に注意
  （とはいえ今回は `location.reload()` を呼ぶだけ）。
- **リスナが居なければ取りこぼす**。受信側（React の `addEventListener`）が張られる前に発火しないよう、
  購読は確実に用意しておく。
- Cargo.toml の `web-sys` features 漏れはコンパイルエラーになる。今回必要だったのは
  `CustomEvent` / `CustomEventInit` / `EventTarget`（[[20260717_game-state-and-clear-notification-impl]]）。
