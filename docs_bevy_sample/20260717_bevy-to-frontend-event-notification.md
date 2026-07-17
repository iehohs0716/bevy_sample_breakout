# Bevy(WASM) からフロント(React)へゲームイベントを通知する

日付: 2026-07-17

WASM 化した Bevy Breakout で、**ゲームオーバー等のイベントを Bevy(Rust) 側からフロント(JS/React)へ
通知**し、「リロードして次ゲーム」「結果画面へ遷移」を実現するための設計調査メモ。
当初は方針整理メモだったが、**第一弾（ゲームクリア → リロード）を実装済み**（下記「実装済み」節）。

前提の連携構成は [[20260711_bevy-wasm-react-integration]]、React→Bevy の初期化パラメータ注入は
[[20260711_react-to-bevy-init-params]] / [[20260711_react-to-bevy-background-injection]] を参照。

---

## 結論

- **可能**。既存は **React → Bevy の一方向**（起動時に `window.__BREAKOUT_CONFIG__` を一度読むだけ）だが、
  その逆方向（**Bevy → フロント通知**）も同じ仕組みの裏返しで実現できる。
- 追加 crate は不要。既に依存に入っている `wasm-bindgen` / `js-sys` / `web-sys`
  （Web ビルド専用 `[target.'cfg(target_arch = "wasm32")'.dependencies]`）だけで足りる。
- WASM 上の Bevy はシングルスレッドなので、Bevy のシステム内から JS を同期的に呼んで問題ない。

## 設計方針: 責務を分離する（重要）

- **Bevy(Rust) の責務は「ゲームオーバーを通知すること」だけ**。Rust から直接
  `location.reload()` / 遷移はしない。遷移を Rust に持たせると、遷移先 URL がビルドに焼き込まれ
  「1 ビルドのままサービスごとに差し替える」既存方針（[[20260711_react-to-bevy-init-params]]）が崩れる。
- **遷移（リロード / 画面移動）は全て React が担う**。Bevy から通知を受け取った React が、
  自分が持つ設定に従って遷移する。
- **リダイレクト先は React 側のパラメータで指定**する（`background` 等と同じく `BevyGame` の prop）。
  サービスごとに遷移先だけ差し替えられる。Bevy はそのURLを知らない。

つまり流れは **Bevy: 通知 → React: 受信 → React: 遷移** の一方向。Rust は `web_sys::window().location()`
に触れない（`Location` feature も不要）。

## やりたいことと最適手段の対応

| やりたいこと | 最適手段 | 補足 |
|---|---|---|
| **リロードして次ゲーム**（想定の主ケース） | Bevy が通知 → React が受信して遷移。次ゲームのパラメータを `__BREAKOUT_CONFIG__` に積んでから React が `location.reload()` | reload 先/次ゲーム設定は React 側パラメータ。ループが既存設計と噛み合う |
| **結果画面へ遷移** | まず **Bevy 内で完結できないか**を検討（Bevy States + UI で結果画面を描画）。React の別画面に出すなら通知 → React が遷移 | WASM 内完結なら JS 連携不要で最もシンプル |

### なぜ「リロード」が手堅いか（重要な制約）
**Bevy(winit) は同一ページ内で再初期化・破棄ができない**（[[20260711_bevy-wasm-react-integration]] の
落とし穴 #2、Bevy Discussion #12195）。React で `BevyGame` を unmount → 再 mount しても Bevy は
起動し直せない。したがって「別ゲーム開始」「盤面フルリセットで再起動」は、
**ページごと作り直すフルリロードが最も安全**。リロードなら winit 破棄問題も自動的に回避される。

---

## 通知手段（3案、上ほど疎結合でおすすめ）

### A. `CustomEvent` を dispatch する（推奨）
Bevy は「イベントを投げるだけ」、React は `addEventListener` で拾うだけ。結合が最も緩い。

```rust
// systems.rs 等。ゲームオーバー判定システム内で呼ぶ（Web ビルド専用）
#[cfg(target_arch = "wasm32")]
fn notify_game_over(score: u32) {
    use wasm_bindgen::JsValue;
    use web_sys::CustomEventInit;

    let detail = js_sys::Object::new();
    js_sys::Reflect::set(&detail, &"score".into(), &JsValue::from_f64(score as f64)).ok();

    let init = CustomEventInit::new();
    init.set_detail(&detail);
    if let (Some(win), Ok(ev)) = (
        web_sys::window(),
        web_sys::CustomEvent::new_with_event_init_dict("breakout:gameover", &init),
    ) {
        win.dispatch_event(&ev).ok();
    }
}
```

React 側が**通知を受けて遷移を実行する**。遷移先は `BevyGame` の prop で受け取る（サービスごとに
差し替え可能）。Rust は URL を一切知らない。

```tsx
// React 側（BevyGame.tsx など）。redirectTo は prop で受け取ったリダイレクト先
useEffect(() => {
  const onGameOver = (e: Event) => {
    const { score } = (e as CustomEvent).detail;
    // 遷移は React が担う。例:
    if (onGameOverRedirect) {
      onGameOverRedirect({ score });        // 呼び出し側にゆだねる（結果画面へ等）
    } else if (redirectTo) {
      window.location.href = redirectTo;      // 次ゲーム/別ページへ
    } else {
      window.location.reload();               // リロードして次ゲーム
    }
  };
  window.addEventListener("breakout:gameover", onGameOver);
  return () => window.removeEventListener("breakout:gameover", onGameOver);
}, [redirectTo, onGameOverRedirect]);
```

`web-sys` の features に **`CustomEvent`, `CustomEventInit`, `EventTarget`** の追加が必要
（現状は `Window` のみ）。**`Location` は不要**（Rust は遷移しないため）。

> web-sys のバージョンにより `CustomEventInit` の設定 API が
> `.detail(&v)`（旧）↔ `.set_detail(&v)`（新, 0.3.70 前後以降）で変わる。ビルドが通らなければ
> どちらかに読み替える。

### B. `window` に載せたコールバックを呼ぶ
React が `window.__BREAKOUT_ON_GAMEOVER__ = (score) => {...}` を仕込み、Bevy が
`js_sys::Reflect::get` で取り出して `Function::call1` する。既存 `__BREAKOUT_CONFIG__` と対称で
流儀は一貫するが、React 側の登録漏れに弱い（未登録だと通知が黙って落ちる）。

### C. `#[wasm_bindgen] extern "C"` で JS 関数を import
`#[wasm_bindgen(module = "...")]` で JS 関数を宣言して直接呼ぶ。型が付くのが利点だが、
繋ぐグルー JS が別途要るので今回の構成にはやや大げさ。

---

## 実装上の勘所

- **一度だけ発火させる**: ゲームオーバー判定は毎フレーム走りうるので、`Resource` やゲーム状態
  （後述の Bevy States）で「通知済みフラグ」を持ち、通知は 1 回に絞る。
- **`#[cfg(target_arch = "wasm32")]` で囲む**: ネイティブビルドでは JS が無いので no-op にする。
  既存 `injection.rs` の分岐（Web 専用 / ネイティブ フォールバック）と同じ流儀に揃える。
- **遷移は React、URL は React 側パラメータ**: Rust は通知のみ。リダイレクト先は `BevyGame` の prop
  （例 `redirectTo` / `onGameOverRedirect`）で受け取り、サービスごとに差し替える。
- **次ゲームのパラメータ受け渡し**: リロード方式なら、React が遷移前に `__BREAKOUT_CONFIG__` を
  次ゲーム用に差し替えてから reload。既存の背景・ブロック配置注入の仕組みをそのまま次ゲーム指定に流用できる。

## 「結果画面を WASM 内で完結」する場合の指針
JS 連携なしで、Bevy の **States**（例: `Playing` / `GameOver`）でゲーム状態を持ち、
`GameOver` 進入時に UI（`ui` feature の Node / Text。既にスコア表示で使用）で結果画面を描画する。
"もう一度" ボタン相当は、盤面の再 spawn（winit 再初期化ではなくエンティティの作り直し）で対応可能。
盤面リセットだけなら winit 制約に触れないため、この範囲は WASM 内で完結できる。

---

## 実装済み（2026-07-17 / 第一弾: ゲームクリア → リロード）

**Rust 側でゲーム状態を Bevy States で管理**し、全ブロック破壊で `gameClear` を通知、React が
リロードする、という一連を実装した。`gameOver` も器だけ用意（遷移条件は将来実装）。

### Rust（`game_engine/`）
- `components.rs`: `GameState { Playing(default), Cleared, GameOver }` を `#[derive(States, ...)]` で定義。
- `notify.rs`（新規）: Bevy→JS 通知モジュール。`notify_game_clear` / `notify_game_over` が
  `window.dispatchEvent(new CustomEvent("breakout:gameclear" | "breakout:gameover", { detail: { score } }))`
  を投げるだけ。**`location` に触れない**（遷移は React）。ネイティブは no-op（`#[cfg]` 分岐）。
- `systems.rs`:
  - `check_game_clear`: `Query<(), With<Brick>>` が空になったら `NextState` を `Cleared` に。
  - `on_game_clear` / `on_game_over`: `OnEnter(...)` で状態進入時に一度だけ通知。
- `main.rs`: `init_state::<GameState>()`。ゲームプレイ system と `check_game_clear` は
  `.run_if(in_state(GameState::Playing))` で**プレイ中のみ**動かす（クリア後はボールが止まる）。
  `OnEnter(Cleared)→on_game_clear`、`OnEnter(GameOver)→on_game_over` を登録。
- `Cargo.toml`: `web-sys` features に `CustomEvent`, `CustomEventInit`, `EventTarget` を追加。

### React（`frontend/`）
- `BevyGame.tsx`: `useEffect` で `breakout:gameclear` / `breakout:gameover` を `addEventListener`。
  - `onGameClear` prop 未指定なら既定で `window.location.reload()`（＝リロードして次ゲーム）。
  - `onGameClear` / `onGameOver` prop を渡せば遷移先を上書きできる（結果画面へ等）。遷移先は
    **React 側パラメータ**で、Rust は URL を知らない。

### 設計上のポイント
- ブロックは `Startup` で spawn 済み → 最初の `Update` フレームには必ず存在するので、
  `check_game_clear` が起動直後に誤って空判定することはない。
- `Cleared` 進入後は `check_game_clear` が `run_if(Playing)` で止まるため二重発火しない。
  `OnEnter` は遷移につき 1 回なので通知も 1 回。リロード後は新しい WASM が `Playing` から始まる。

### 検証状況
- `cargo check`（ネイティブ）/ `cargo check --target wasm32-unknown-unknown` / フロント `tsc --noEmit`
  いずれも成功。
- **実ブラウザでの clear→reload の目視確認は未実施**（全ブロック破壊まで実際にプレイする必要があり
  自動化が難しい）。[[verify-in-real-browser]] に従い、次は実ブラウザでの確認が必要。

## 検証観点（実装時）
[[20260711_wasm-bevy-browser-verification]] と同様、**実ブラウザで発火を確認**すること
（[[verify-in-real-browser]]）。具体的には:
- devtools コンソールで `window.addEventListener("breakout:gameover", e => console.log(e.detail))`
  を仕込み、ゲームオーバー時に 1 回だけログが出るか。
- リロード方式なら、**React 側**の reload 後に次ゲームのパラメータが反映されているか
  （Rust は遷移していないこと）。
