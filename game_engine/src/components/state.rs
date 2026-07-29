//! ゲーム全体の状態遷移（`GameState`）の定義。

use bevy::prelude::*;

/// ゲーム全体の状態を Bevy の States で管理する。状態遷移は Rust 側が担い、
/// 状態に入った瞬間（`OnEnter`）に初期化やフロント(JS)への通知を行う（`systems::notify` 参照）。
/// - `GameStart`  : 開始待ち（初期状態）。ボールはクリックまで静止し、左クリックで `Playing` へ。
///                  初回の盤面は `setup`(Startup) が用意する。この状態の OnEnter では何もしない。
/// - `Playing`    : プレイ中。ゲームプレイ system はこの状態でのみ動く。
/// - `Cleared`    : 全ブロックを破壊した（クリア）。進入時に `breakout:gameclear` を通知(WASMビルド時)。
/// - `GameOver`   : ゲームオーバー（ライフ 0）。進入時、ネイティブは `GameRestart` へ遷移、
///                  WASM は `breakout:gameover` を通知(WASMビルド時)。
/// - `GameRestart`: 敗北後の再スタート＝クリック待ち（ネイティブのみ）。進入時に `reset_game` が
///                  盤面を作り直し、この状態のままクリックを待つ（`GameStart` には戻さない）。
///                  起動後にしか入らないので初期化の順序問題が無い。`GameStart` と役割を分けたまま保つ。
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    GameStart,
    Playing,
    Cleared,
    GameOver,
    GameRestart,
}
