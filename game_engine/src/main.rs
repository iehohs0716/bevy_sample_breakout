//! A simplified implementation of the classic game "Breakout".
//!
//! Demonstrates Bevy's stepping capabilities if compiled with the `bevy_debug_stepping` feature.
//!
//! モジュール構成:
//! - `config`: ゲーム全体の定数
//! - `components`: Component / Resource / Event の定義
//! - `injection`: React(JS) から渡される初期化パラメータの読み取り
//! - `notify`: ゲームイベント（クリア等）をフロント(JS)へ通知
//! - `rendering`: 画像フィット計算とブロック描画ヘルパー
//! - `setup`: 起動時セットアップ system
//! - `systems`: 毎フレームのゲームプレイ system

mod components;
mod config;
mod injection;
mod notify;
mod rendering;
mod setup;
mod systems;

use bevy::prelude::*;

use components::{GameState, Lives, Score};
use config::INITIAL_LIVES;
use injection::{
    injected_background_image, injected_brick_image, injected_brick_layout, BackgroundOverride,
    BrickImageOverride, BrickLayoutOverride,
};
use setup::setup;
use systems::{
    apply_velocity, check_for_collisions, check_game_clear, launch_ball_on_click, move_paddle,
    on_game_clear, on_game_over, play_collision_sound, reset_game, update_lives, update_scoreboard,
};

fn main() {
    App::new()
        .insert_resource(BackgroundOverride(injected_background_image()))
        .insert_resource(BrickLayoutOverride(injected_brick_layout()))
        .insert_resource(BrickImageOverride(injected_brick_image()))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                // Web ビルド時はこの ID の canvas 要素に描画する。
                // React 側の `<canvas id="bevy-canvas">` と一致させること。
                // ネイティブ実行時はこの指定は無視される。
                canvas: Some("#bevy-canvas".into()),
                // canvas を親要素のサイズにフィットさせる（React レイアウト側で制御可能に）。
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(Score(0))
        .insert_resource(Lives(INITIAL_LIVES))
        // 背景画像を比率維持で置くため、余白（レターボックス）は黒で塗る。
        .insert_resource(ClearColor(Color::BLACK))
        // ゲーム状態を Rust 側で管理する（初期状態は Playing）。
        .init_state::<GameState>()
        .add_systems(Startup, setup)
        // Add our simulation systems to the update schedule
        // which is called once per frame.
        // ゲームプレイ system はプレイ中（Playing）のみ動かす。クリア後はボールを止める。
        .add_systems(
            Update,
            (apply_velocity, move_paddle, check_for_collisions)
                // `chain`ing systems together runs them in order
                .chain()
                .run_if(in_state(GameState::Playing)),
        )
        // 全ブロック破壊の判定もプレイ中のみ。0 になったら Cleared へ遷移する。
        .add_systems(Update, check_game_clear.run_if(in_state(GameState::Playing)))
        .add_systems(Update, (update_scoreboard, update_lives))
        // GameStart に入るたびに盤面をリセットし、クリックでボール発射 → Playing へ。
        .add_systems(OnEnter(GameState::GameStart), reset_game)
        .add_systems(
            Update,
            launch_ball_on_click.run_if(in_state(GameState::GameStart)),
        )
        // 状態に入った瞬間に一度だけ、通知や再スタート処理を行う。
        .add_systems(OnEnter(GameState::Cleared), on_game_clear)
        .add_systems(OnEnter(GameState::GameOver), on_game_over)
        .add_observer(play_collision_sound)
        .run();
}
