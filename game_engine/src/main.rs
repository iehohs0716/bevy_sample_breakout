//! A simplified implementation of the classic game "Breakout".
//!
//! Demonstrates Bevy's stepping capabilities if compiled with the `bevy_debug_stepping` feature.
//!
//! モジュール構成:
//! - `util`: アスペクト比を保ったまま内接させる `contain_fit`（ドメイン型に非依存）
//! - `common`: ブロックの spawn ヘルパー。`Query` を取らない、手動で呼び出す処理だけを持つ
//!   （`systems::setup` と `systems::reset_game` の両方から使われるが、中身は全てブロックという
//!   単一ドメインの処理）。`Query` を取る実際の Bevy system は `systems::update::brick` に置く
//! - `config`: ゲーム全体の定数
//! - `components`: Component / Resource / Event の定義
//! - `injection`: React(JS) から渡される初期化パラメータの読み取り
//! - `systems`: Bevy system への入り口。起動時に一度だけ走る `Startup` system は
//!   `systems::setup`、毎フレーム／衝突イベントへの反応は `systems::update`、
//!   終端状態（クリア／ゲームオーバー）突入時の JS 通知は `systems::terminate` に分離している

mod util;
mod common;
mod components;
mod config;
mod injection;
mod systems;

use bevy::prelude::*;

use components::{GameState, Lives, Score};
use config::INITIAL_LIVES;
use injection::{
    injected_background_image, injected_brick_image, injected_brick_layout, BackgroundOverride,
    BrickImageOverride, BrickLayoutOverride,
};
use systems::{
    apply_velocity_to_transform_object, check_ball_brick_collision, check_ball_deathzone_collision,
    check_ball_paddle_collision, check_ball_wall_collision, check_game_clear,
    launch_ball_on_click, mark_broken_edges_on_brick_destroyed, move_paddle, on_game_clear,
    on_game_over, play_collision_sound, redraw_broken_bricks, reset_game, setup, update_lives,
    update_scoreboard,
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
            (
                apply_velocity_to_transform_object,
                move_paddle,
                // ボールとの当たり判定は「相手が何か」で4つのsystemに分けている
                check_ball_brick_collision,
                check_ball_wall_collision,
                check_ball_paddle_collision,
                check_ball_deathzone_collision,
            )
                // `chain`ing systems together runs them in order
                .chain()
                .run_if(in_state(GameState::Playing)),
        )
        // 全ブロック破壊の判定もプレイ中のみ。0 になったら Cleared へ遷移する。
        .add_systems(Update, check_game_clear.run_if(in_state(GameState::Playing)))
        .add_systems(Update, (update_scoreboard, update_lives))
        // ブロック破壊で更新された `BrokenEdges` を同フレームで反映するため衝突判定の後に走らせる。
        .add_systems(Update, redraw_broken_bricks.after(check_ball_brick_collision))
        // クリック待ち（GameStart=初回 / GameRestart=敗北後）中の左クリックでボール発射 → Playing へ。
        .add_systems(
            Update,
            launch_ball_on_click
                .run_if(in_state(GameState::GameStart).or_else(in_state(GameState::GameRestart))),
        )
        // 状態に入った瞬間に一度だけ、通知や再スタート処理を行う。
        .add_systems(OnEnter(GameState::Cleared), on_game_clear)
        .add_systems(OnEnter(GameState::GameOver), on_game_over)
        // 敗北後の再スタート（ネイティブのみ）: 盤面を作り直して GameStart へ戻す。
        .add_systems(OnEnter(GameState::GameRestart), reset_game)
        .add_observer(play_collision_sound)
        .add_observer(mark_broken_edges_on_brick_destroyed)
        .run();
}
