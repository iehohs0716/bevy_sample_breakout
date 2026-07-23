//! 毎フレーム走るゲームプレイ system（パドル移動・速度適用・スコア更新・衝突判定）と
//! 衝突音の再生。

use bevy::{
    math::bounding::{Aabb2d, BoundingCircle, BoundingVolume, IntersectsVolume},
    prelude::*,
};

use crate::components::{
    Ball, BallCollided, Brick, Collider, CollisionSound, DeathZone, GameAssets, GameState, Lives,
    LivesUi, Paddle, Score, ScoreboardUi, Velocity,
};
use crate::notify::{notify_game_clear, notify_game_over};
use crate::rendering::spawn_brick;
use crate::config::{
    BALL_DIAMETER, BALL_SPEED, BALL_STARTING_POSITION, INITIAL_BALL_DIRECTION, INITIAL_LIVES,
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

pub fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
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
    *writer.text(*score_root, 1) = score.to_string();
}

pub fn update_lives(
    lives: Res<Lives>,
    lives_root: Single<Entity, (With<LivesUi>, With<Text>)>,
    mut writer: TextUiWriter,
) {
    *writer.text(*lives_root, 1) = lives.to_string();
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
    for position in &game_assets.brick_layout.positions {
        spawn_brick(
            &mut commands,
            *position,
            game_assets.brick_layout.cell_size,
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

pub fn check_for_collisions(
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut lives: ResMut<Lives>,
    mut next_state: ResMut<NextState<GameState>>,
    ball_query: Single<(&mut Velocity, &mut Transform), With<Ball>>,
    // ball_query が Transform を `&mut` で触るため、Collider 側の `&Transform` と競合しないよう
    // `Without<Ball>` で両クエリを排他にする（ボールは Collider を持たないので実データは変わらない）。
    collider_query: Query<
        (Entity, &Transform, Option<&Brick>, Option<&DeathZone>),
        (With<Collider>, Without<Ball>),
    >,
) {
    let (mut ball_velocity, mut ball_transform) = ball_query.into_inner();

    for (collider_entity, collider_transform, maybe_brick, maybe_death) in &collider_query {
        let collision = ball_collision(
            BoundingCircle::new(ball_transform.translation.truncate(), BALL_DIAMETER / 2.),
            Aabb2d::new(
                collider_transform.translation.truncate(),
                collider_transform.scale.truncate() / 2.,
            ),
        );

        if let Some(collision) = collision {
            // Trigger observers of the "BallCollided" event
            commands.trigger(BallCollided);

            // 下端（DeathZone）に触れたらライフを 1 減らす。反射はさせない。
            // - 残りライフがあればボールを初期位置・初速に戻して続行する。
            // - 0 になったら GameOver へ遷移する（ボールはそのフレーム以降、
            //   `run_if(in_state(Playing))` により停止する）。
            if maybe_death.is_some() {
                lives.0 = lives.0.saturating_sub(1);
                if lives.0 == 0 {
                    next_state.set(GameState::GameOver);
                } else {
                    ball_transform.translation = BALL_STARTING_POSITION;
                    ball_velocity.0 = INITIAL_BALL_DIRECTION.normalize() * BALL_SPEED;
                }
                // ボールをリセットしたので、このフレームの残りの衝突判定は打ち切る。
                break;
            }

            // Bricks should be despawned and increment the scoreboard on collision
            if maybe_brick.is_some() {
                commands.entity(collider_entity).despawn();
                score.0 += 1;
            }

            // Reflect the ball's velocity when it collides
            let mut reflect_x = false;
            let mut reflect_y = false;

            // Reflect only if the velocity is in the opposite direction of the collision
            // This prevents the ball from getting stuck inside the bar
            match collision {
                Collision::Left => reflect_x = ball_velocity.x > 0.0,
                Collision::Right => reflect_x = ball_velocity.x < 0.0,
                Collision::Top => reflect_y = ball_velocity.y < 0.0,
                Collision::Bottom => reflect_y = ball_velocity.y > 0.0,
            }

            // Reflect velocity on the x-axis if we hit something on the x-axis
            if reflect_x {
                ball_velocity.x = -ball_velocity.x;
            }

            // Reflect velocity on the y-axis if we hit something on the y-axis
            if reflect_y {
                ball_velocity.y = -ball_velocity.y;
            }
        }
    }
}

/// 全ブロックが無くなったらクリア状態へ遷移する。`Playing` 中のみ動作させる想定
/// （ブロックは `Startup` で spawn 済みなので、最初の `Update` フレームには存在する）。
/// 実際の JS 通知は状態遷移側（`OnEnter(GameState::Cleared)` → `on_game_clear`）で行う。
pub fn check_game_clear(
    bricks: Query<(), With<Brick>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if bricks.is_empty() {
        next_state.set(GameState::Cleared);
    }
}

/// クリア状態に入った瞬間に一度だけ、フロント(JS)へゲームクリアを通知する。
/// `OnEnter(GameState::Cleared)` に登録するので、状態遷移につき 1 回だけ走る。
pub fn on_game_clear(score: Res<Score>) {
    notify_game_clear(score.0);
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
    notify_game_over(score.0);

    #[cfg(not(target_arch = "wasm32"))]
    next_state.set(GameState::GameRestart);
}

pub fn play_collision_sound(
    _collided: On<BallCollided>,
    mut commands: Commands,
    sound: Res<CollisionSound>,
) {
    commands.spawn((AudioPlayer(sound.clone()), PlaybackSettings::DESPAWN));
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum Collision {
    Left,
    Right,
    Top,
    Bottom,
}

// Returns `Some` if `ball` collides with `bounding_box`.
// The returned `Collision` is the side of `bounding_box` that `ball` hit.
fn ball_collision(ball: BoundingCircle, bounding_box: Aabb2d) -> Option<Collision> {
    if !ball.intersects(&bounding_box) {
        return None;
    }

    let closest = bounding_box.closest_point(ball.center());
    let offset = ball.center() - closest;
    let side = if offset.x.abs() > offset.y.abs() {
        if offset.x < 0. {
            Collision::Left
        } else {
            Collision::Right
        }
    } else if offset.y > 0. {
        Collision::Top
    } else {
        Collision::Bottom
    };

    Some(side)
}
