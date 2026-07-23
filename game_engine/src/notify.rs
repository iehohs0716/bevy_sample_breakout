//! Bevy(WASM) から フロント(JS/React) へゲームイベントを通知する。
//!
//! 責務分離: **Rust は「イベントを投げるだけ」**で、リロードや画面遷移は一切しない
//! （`location` に触れない）。通知を受け取った React が、自分の持つパラメータ（遷移先）に
//! 従って遷移する。これにより「アプリのコード（Rust/WASM）は 1 ビルド」のまま、遷移先を
//! サービスごとに React 側で差し替えられる（背景・ブロック注入と同じ思想 → `injection` 参照）。
//!
//! 通知は `window.dispatchEvent(new CustomEvent(name, { detail: { score } }))` で行い、
//! React は `window.addEventListener(name, ...)` で受ける。ネイティブビルドでは JS が無いので
//! すべて no-op。

/// ゲームクリア（全ブロック破壊）を JS に通知する。
/// React 側は `window.addEventListener("breakout:gameclear", ...)` で受けて遷移する。
/// `detail.result` は `"clear"`（`breakout:gameover` の `"gameover"` と区別できる）。
#[cfg(target_arch = "wasm32")]
pub fn notify_game_clear(score: usize) {
    dispatch_event("breakout:gameclear", "clear", score);
}

/// ネイティブビルドでは JS が無いので no-op。
#[cfg(not(target_arch = "wasm32"))]
pub fn notify_game_clear(_score: usize) {}

/// ゲームオーバー（ライフ 0）を JS に通知する。
/// React 側は `window.addEventListener("breakout:gameover", ...)` で受けて遷移する。
/// `detail.result` は `"gameover"`（クリアの `"clear"` と区別できる）。
#[cfg(target_arch = "wasm32")]
pub fn notify_game_over(score: usize) {
    dispatch_event("breakout:gameover", "gameover", score);
}

/// ネイティブビルドでは JS が無いので no-op。
#[cfg(not(target_arch = "wasm32"))]
pub fn notify_game_over(_score: usize) {}

/// Web ビルド専用。`window` に `CustomEvent`（`detail: { result, score }`）を dispatch する共通処理。
/// `result` はクリア/ゲームオーバーを区別する属性（`"clear"` / `"gameover"`）。イベント名でも
/// 区別できるが、`detail.result` を見れば 1 つのハンドラでまとめて分岐できる。
/// window が取れない / イベント生成に失敗した場合は warn するだけで、ゲーム自体は続行する。
#[cfg(target_arch = "wasm32")]
fn dispatch_event(name: &str, result: &str, score: usize) {
    use bevy::prelude::warn;
    use wasm_bindgen::JsValue;
    use web_sys::CustomEventInit;

    let Some(window) = web_sys::window() else {
        warn!("window が取得できないため {name} を通知できません");
        return;
    };

    // detail に { result, score } を載せる。React 側は `e.detail.result` / `e.detail.score` で読める。
    let detail = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &detail,
        &JsValue::from_str("result"),
        &JsValue::from_str(result),
    );
    let _ = js_sys::Reflect::set(
        &detail,
        &JsValue::from_str("score"),
        &JsValue::from_f64(score as f64),
    );

    let init = CustomEventInit::new();
    init.set_detail(&detail);

    match web_sys::CustomEvent::new_with_event_init_dict(name, &init) {
        Ok(event) => {
            let _ = window.dispatch_event(&event);
        }
        Err(err) => warn!("CustomEvent {name} の生成に失敗しました: {err:?}"),
    }
}
