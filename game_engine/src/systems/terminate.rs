//! ゲームが終端状態（クリア／ゲームオーバー）に入った瞬間（`OnEnter`）に一度だけ、
//! フロント(JS)へゲームイベントを通知する system。JS 通知の実装（`window.dispatchEvent` 等）
//! はこのモジュールの非公開ヘルパーとして持つ。呼び出し元は `on_game_clear` / `on_game_over`
//! の 2 つの system だけを知っていればよく、通知の仕組みそのものは `systems::terminate` の
//! 外からは見えない。

use bevy::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
use crate::components::GameState;
use crate::components::Score;

/// クリア状態に入った瞬間に一度だけ、フロント(JS)へゲームクリアを通知する。
/// `OnEnter(GameState::Cleared)` に登録するので、状態遷移につき 1 回だけ走る。
pub fn on_game_clear(
    score: Res<Score>,
    #[cfg(not(target_arch = "wasm32"))] mut next_state: ResMut<NextState<GameState>>,
) {
    dispatch_event("breakout:gameclear", "clear", score.0);

    #[cfg(not(target_arch = "wasm32"))]
    next_state.set(GameState::GameRestart);
}

/// ゲームオーバー状態に入った瞬間に一度だけ実行する。`OnEnter(GameState::GameOver)` に登録。
/// - WASM: `breakout:gameover` を通知し、遷移は React に委ねる（クリアと同じ思想）。
/// - ネイティブ: JS 通知は no-op なのでそのままだと画面が固まる。代わりに `GameRestart` へ遷移し、
///   `reset_game` で盤面を作り直して再プレイできるようにする。
/// `next_state` はネイティブでのみ使うため、WASM では引数ごと省く（未使用警告の回避）。
pub fn on_game_over(
    score: Res<Score>,
    #[cfg(not(target_arch = "wasm32"))] mut next_state: ResMut<NextState<GameState>>,
) {
    dispatch_event("breakout:gameover", "gameover", score.0);

    #[cfg(not(target_arch = "wasm32"))]
    next_state.set(GameState::GameRestart);
}

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

    // web_sysのカスタムイベントを用いた、イベントの汎用発火コード
    match web_sys::CustomEvent::new_with_event_init_dict(name, &init) {
        Ok(event) => {
            let _ = window.dispatch_event(&event);
        }
        Err(err) => warn!("CustomEvent {name} の生成に失敗しました: {err:?}"),
    }
}
