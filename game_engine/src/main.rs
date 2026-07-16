//! A simplified implementation of the classic game "Breakout".
//!
//! Demonstrates Bevy's stepping capabilities if compiled with the `bevy_debug_stepping` feature.
//!
//! モジュール構成:
//! - `config`: ゲーム全体の定数
//! - `components`: Component / Resource / Event の定義
//! - `injection`: React(JS) から渡される初期化パラメータの読み取り
//! - `rendering`: 画像フィット計算とブロック描画ヘルパー
//! - `setup`: 起動時セットアップ system
//! - `systems`: 毎フレームのゲームプレイ system

mod components;
mod config;
mod injection;
mod rendering;
mod setup;
mod systems;

use bevy::prelude::*;

use components::Score;
use injection::{
    injected_background_image, injected_brick_image, injected_brick_layout, BackgroundOverride,
    BrickImageOverride, BrickLayoutOverride,
};
use setup::setup;
use systems::{
    apply_velocity, check_for_collisions, move_paddle, play_collision_sound, update_scoreboard,
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
        // 背景画像を比率維持で置くため、余白（レターボックス）は黒で塗る。
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Startup, setup)
        // Add our simulation systems to the update schedule
        // which is called once per frame.
        .add_systems(
            Update,
            (apply_velocity, move_paddle, check_for_collisions)
                // `chain`ing systems together runs them in order
                .chain(),
        )
        .add_systems(Update, update_scoreboard)
        .add_observer(play_collision_sound)
        .run();
}
