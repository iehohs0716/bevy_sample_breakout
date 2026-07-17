# ゲーム状態管理（Bevy States）とクリア通知の実装解説

日付: 2026-07-17

Breakout で「全ブロックを破壊したら Bevy(Rust) がゲームクリアを検知し、フロント(React) へ通知して
リロードする」機能の**実装の中身**をコードに沿って解説する。

- 設計方針・なぜこの形か: [[20260717_bevy-to-frontend-event-notification]]
- `window.dispatch_event(&event)` が動く仕組みの詳細: [[20260717_wasm-dispatch-event-mechanism]]
- 前提の連携構成: [[20260711_bevy-wasm-react-integration]]

---

## 全体の流れ

```
[Bevy/Rust]                                   [React/JS]
Playing ──全ブロック破壊──▶ Cleared
   │ check_game_clear             │ OnEnter(Cleared)
   │ (Query<Brick> が空?)         │ on_game_clear
   │                              ▼
   │                     notify_game_clear(score)
   │                     window.dispatchEvent(
   │                       new CustomEvent("breakout:gameclear",
   │                         { detail: { score } }))  ──────▶ addEventListener("breakout:gameclear")
   │                                                             └─ location.reload()  ← 遷移は React
```

役割分担（[[20260717_bevy-to-frontend-event-notification]] の設計方針そのまま）:
- **Rust**: 状態を管理し、状態遷移時に「イベントを投げるだけ」。`location` に触れない。
- **React**: 通知を受けて遷移する。遷移先は React 側のパラメータ（prop）。

---

## Rust 側（`game_engine/`）

### 1. 状態の定義 — `components.rs`
ゲーム全体の状態を Bevy の **States** で持つ。`Playing` が初期状態。

```rust
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Playing,
    Cleared,
    GameOver,
}
```

`States` を derive すると、`init_state` で登録でき、`in_state(...)` / `OnEnter(...)` /
`NextState<GameState>` が使えるようになる。`Default` が初期状態になる。

### 2. 通知モジュール — `notify.rs`（新規）
Bevy→JS 通知の入口。**Rust は dispatch するだけ**で、リロードや遷移はしない。
ネイティブビルドでは JS が無いので `#[cfg]` で no-op に分岐する（`injection.rs` と同じ流儀）。

```rust
#[cfg(target_arch = "wasm32")]
pub fn notify_game_clear(score: usize) { dispatch_event("breakout:gameclear", score); }
#[cfg(not(target_arch = "wasm32"))]
pub fn notify_game_clear(_score: usize) {}      // ネイティブは何もしない

#[cfg(target_arch = "wasm32")]
fn dispatch_event(name: &str, score: usize) {
    let Some(window) = web_sys::window() else { /* warn して return */ };
    let detail = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&detail, &"score".into(), &JsValue::from_f64(score as f64));
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&detail);
    if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(name, &init) {
        let _ = window.dispatch_event(&event);
    }
}
```

`notify_game_over` も同様に用意（`"breakout:gameover"`）。現状 `GameOver` への遷移条件は無いので
発火しないが、将来（ボール落下等）に備えた器。この 1 行 `window.dispatch_event(&event)` が何を
しているかは [[20260717_wasm-dispatch-event-mechanism]] で詳説。

### 3. 判定と通知の system — `systems.rs`
- **判定**: ブロックが 1 つも無くなったら `Cleared` へ遷移させる。状態を変えるだけで通知はしない
  （通知は状態進入時に一度だけ行うため、責務を分ける）。

```rust
pub fn check_game_clear(
    bricks: Query<(), With<Brick>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if bricks.is_empty() {
        next_state.set(GameState::Cleared);
    }
}
```

- **通知**: 状態に入った瞬間に一度だけ走らせる。

```rust
pub fn on_game_clear(score: Res<Score>) { notify_game_clear(score.0); }
pub fn on_game_over(score: Res<Score>)  { notify_game_over(score.0); }
```

### 4. 配線 — `main.rs`
```rust
.init_state::<GameState>()
// ゲームプレイと判定は「プレイ中」だけ動かす（クリア後はボールが止まる）
.add_systems(Update,
    (apply_velocity, move_paddle, check_for_collisions).chain()
        .run_if(in_state(GameState::Playing)))
.add_systems(Update, check_game_clear.run_if(in_state(GameState::Playing)))
// 状態に入った瞬間に一度だけ通知
.add_systems(OnEnter(GameState::Cleared),  on_game_clear)
.add_systems(OnEnter(GameState::GameOver), on_game_over)
```

### 5. Cargo.toml
`web-sys` の features に、通知で使う DOM API を追加。
```toml
web-sys = { version = "0.3", features = ["Window", "CustomEvent", "CustomEventInit", "EventTarget"] }
```
`CustomEvent` / `CustomEventInit` はイベント生成、`EventTarget` は `dispatch_event`（`Window` が
`EventTarget` を継承）に必要。`Location` は**入れていない**（Rust は遷移しないため）。

---

## React 側（`frontend/src/components/BevyGame.tsx`）

`useEffect` で 2 つのイベントを購読し、クリーンアップで解除する。**遷移は React が担当**。

```tsx
useEffect(() => {
  const handleGameClear = (e: Event) => {
    const detail = (e as CustomEvent<{ score: number }>).detail;
    if (onGameClear) onGameClear(detail);
    else window.location.reload();          // 既定: リロードして次ゲーム
  };
  const handleGameOver = (e: Event) => onGameOver?.((e as CustomEvent<{score:number}>).detail);
  window.addEventListener("breakout:gameclear", handleGameClear);
  window.addEventListener("breakout:gameover", handleGameOver);
  return () => {
    window.removeEventListener("breakout:gameclear", handleGameClear);
    window.removeEventListener("breakout:gameover", handleGameOver);
  };
}, [onGameClear, onGameOver]);
```

- `onGameClear` / `onGameOver` は **prop**。未指定なら clear は `location.reload()`。
  結果画面へ飛ばしたい等は prop で上書き（遷移先 URL は React 側パラメータ）。
- WASM 起動の `useEffect`（`startedRef` ガード付き）とは別 effect にしている。イベント購読は
  StrictMode の二重実行でも `removeEventListener` で綺麗に解除されるため冪等。

---

## 設計上の勘所（なぜ壊れないか）

- **起動直後の誤判定なし**: ブロックは `Startup` で spawn され、その Commands は最初の `Update` の
  前にフラッシュされる。よって `check_game_clear` の初回実行時にはブロックが必ず存在する。
- **二重発火なし**: `Cleared` に入ると `check_game_clear` は `run_if(Playing)` で止まる。`OnEnter` は
  状態遷移につき 1 回なので、通知も 1 回だけ。リロード後は新しい WASM が `Playing` から再開する。
- **ネイティブ実行を壊さない**: `notify.rs` は `#[cfg(not(target_arch = "wasm32"))]` 側が no-op。
  `cargo run`（ネイティブ）では通知が単に呼ばれないだけで、状態管理はそのまま動く。

## 検証状況
`cargo check`（ネイティブ / `wasm32-unknown-unknown`）と フロント `tsc --noEmit` は成功。
実ブラウザでの clear→reload 目視確認は未実施（全ブロック破壊まで手動プレイが必要）。
→ [[verify-in-real-browser]] に従い実機確認が残タスク。
