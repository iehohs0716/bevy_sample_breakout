//! 毎フレーム走るゲームプレイ system（パドル移動・速度適用・スコア更新）と衝突音の再生。
//! 起動時に一度だけ走る `Startup` system は子モジュール `setup` に、`Update` スケジュールで
//! 実行される system（ボール衝突判定・ブロック単一ドメインの system・observer）は子モジュール
//! `update` に、ゲーム終端（クリア／ゲームオーバー）時の JS 通知（通知の実装込み）は子モジュール
//! `terminate` に分離し、ここで `pub use` して外（`main`）からは `systems::` 経由でまとめて
//! 見えるようにしている（`main` は `systems` だけを読み、`systems` が `setup` / `update` /
//! `terminate` を読むという依存の向きにするため）。

mod setup;
pub use setup::setup;

mod update;
pub use update::{
    check_ball_brick_collision, check_ball_deathzone_collision, check_ball_paddle_collision,
    check_ball_wall_collision, check_game_clear, mark_broken_edges_on_brick_destroyed,
    redraw_broken_bricks,
};

mod terminate;
pub use terminate::{on_game_clear, on_game_over};

use bevy::prelude::*;

use crate::components::{
    Ball, BallCollided, Brick, CollisionSound, GameAssets, GameState, Lives, LivesUi, Paddle,
    Score, ScoreboardUi, Velocity,
};
use crate::common::{spawn_brick, BrickAssets};
use crate::config::{
    BALL_SPEED, BALL_STARTING_POSITION, INITIAL_BALL_DIRECTION, INITIAL_LIVES,
    LEFT_WALL, PADDLE_PADDING, PADDLE_SIZE, PADDLE_SPEED, RIGHT_WALL, WALL_THICKNESS,
};

pub fn move_paddle(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut paddle_transform: Single<&mut Transform, With<Paddle>>,
    time: Res<Time>,
) {
    let mut direction = 0.0;

    if keyboard_input.pressed(KeyCode::ArrowLeft) {
        direction -= 1.0;
    }

    if keyboard_input.pressed(KeyCode::ArrowRight) {
        direction += 1.0;
    }

    // Calculate the new horizontal paddle position based on player input
    let new_paddle_position =
        paddle_transform.translation.x + direction * PADDLE_SPEED * time.delta_secs();

    // Update the paddle position,
    // making sure it doesn't cause the paddle to leave the arena
    let left_bound = LEFT_WALL + WALL_THICKNESS / 2.0 + PADDLE_SIZE.x / 2.0 + PADDLE_PADDING;
    let right_bound = RIGHT_WALL - WALL_THICKNESS / 2.0 - PADDLE_SIZE.x / 2.0 - PADDLE_PADDING;

    paddle_transform.translation.x = new_paddle_position.clamp(left_bound, right_bound);
}

pub fn apply_velocity_to_transform_object(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_secs();
        transform.translation.y += velocity.y * time.delta_secs();
    }
}

pub fn update_scoreboard(
    score: Res<Score>,
    score_root: Single<Entity, (With<ScoreboardUi>, With<Text>)>,
    mut writer: TextUiWriter,
) {
    // `*score_root` でも動くが、rust-analyzer が Single の Deref を解決できず E0614 を誤検知する。
    // `into_inner()` で中身の Entity を取り出せば * を踏まないので誤検知しない（rustc は両方通る）。
    *writer.text(score_root.into_inner(), 1) = score.to_string();
}

pub fn update_lives(
    lives: Res<Lives>,
    lives_root: Single<Entity, (With<LivesUi>, With<Text>)>,
    mut writer: TextUiWriter,
) {
    *writer.text(lives_root.into_inner(), 1) = lives.to_string();
}

/// 敗北後の再スタート処理。`OnEnter(GameState::GameRestart)` に登録する（ネイティブのみ経由）。
/// スコア/ライフをリセットし、ブロックを配置し直し、ボールを初期位置で静止させる。
/// **状態は `GameRestart` のまま**にする（＝敗北後のクリック待ち状態そのもの）。ここで起動用の
/// `GameStart` に戻さないのが肝で、それにより「起動時」と「再スタート時」を最後まで別状態に保つ。
/// クリックでの発射は `launch_ball_on_click` が `GameStart` / `GameRestart` の両方で担う。
///
/// `GameRestart` は起動後（`GameOver` 経由）にしか入らないため、`GameAssets` も `Ball` も必ず存在
/// する。初期状態 `GameStart` の OnEnter のように Startup より前に走ることが無いので、`Option`
/// ガードや空振り処理は不要で、`Res` / `Single` を素直に使える。
pub fn reset_game(
    mut commands: Commands,
    mut brick_assets: BrickAssets,
    mut score: ResMut<Score>,
    mut lives: ResMut<Lives>,
    game_assets: Res<GameAssets>,
    bricks: Query<Entity, With<Brick>>,
    ball: Single<(&mut Transform, &mut Velocity), With<Ball>>,
) {
    score.0 = 0;
    lives.0 = INITIAL_LIVES;

    // 残っているブロックを消してから、確定済みレイアウトで配置し直す。
    for entity in &bricks {
        commands.entity(entity).despawn();
    }
    for (position, cell) in game_assets
        .brick_layout
        .positions
        .iter()
        .zip(&game_assets.brick_layout.cells)
    {
        spawn_brick(
            &mut commands,
            &mut brick_assets,
            *position,
            game_assets.brick_layout.cell_size,
            *cell,
            game_assets.brick_image.clone(),
        );
    }

    // ボールを初期位置で静止させる。状態は GameRestart のまま＝このままクリック待ち。
    let (mut transform, mut velocity) = ball.into_inner();
    transform.translation = BALL_STARTING_POSITION;
    velocity.0 = Vec2::ZERO;
}

/// `GameStart` / `GameRestart`（どちらもクリック待ち）中に左クリックされたらボールを発射し、`Playing` へ遷移する。
/// 初回開始も再スタートも「クリックで動き出す」流れを共通化する。
pub fn launch_ball_on_click(
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut ball_velocity: Single<&mut Velocity, With<Ball>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if mouse_input.just_pressed(MouseButton::Left) {
        ball_velocity.0 = INITIAL_BALL_DIRECTION.normalize() * BALL_SPEED;
        next_state.set(GameState::Playing);
    }
}

pub fn play_collision_sound(
    _collided: On<BallCollided>,
    mut commands: Commands,
    sound: Res<CollisionSound>,
) {
    commands.spawn((AudioPlayer(sound.clone()), PlaybackSettings::DESPAWN));
}
