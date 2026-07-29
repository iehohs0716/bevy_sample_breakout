//! ボールと他のエンティティ（ブロック・壁・パドル・デスゾーン）の当たり判定 system と、
//! それを支える汎用の当たり判定ヘルパー（`BoundingBoxSource` / `BoundingCircleSource`）。

use bevy::{
    math::bounding::{Aabb2d, BoundingCircle, BoundingVolume, IntersectsVolume},
    prelude::*,
};

use crate::components::{
    Ball, BallCollided, Brick, BrickDestroyed, DeathZone, GameState, Lives, Paddle, Score,
    Velocity, Wall,
};
use crate::config::{
    BALL_SPEED, BALL_STARTING_POSITION, BOTTOM_WALL, GAP_BETWEEN_PADDLE_AND_FLOOR,
    INITIAL_BALL_DIRECTION, PADDLE_SIZE,
};

/// 衝突した辺に応じてボールの速度を反射させる。壁・パドルなど「ぶつかったら跳ね返るだけで
/// 副作用が無い」衝突すべてで共通の処理。ブロック衝突・デスゾーン衝突もこの反射自体は使うが、
/// スコア加算やライフ減少という追加の副作用を持つのでそれぞれの system 側に直接書いている。
fn reflect_ball_velocity(ball_velocity: &mut Velocity, collision: Collision) {
    // Reflect only if the velocity is in the opposite direction of the collision
    // This prevents the ball from getting stuck inside the bar
    let (mut reflect_x, mut reflect_y) = (false, false);
    match collision {
        Collision::Left => reflect_x = ball_velocity.x > 0.0,
        Collision::Right => reflect_x = ball_velocity.x < 0.0,
        Collision::Top => reflect_y = ball_velocity.y < 0.0,
        Collision::Bottom => reflect_y = ball_velocity.y > 0.0,
    }

    if reflect_x {
        ball_velocity.x = -ball_velocity.x;
    }
    if reflect_y {
        ball_velocity.y = -ball_velocity.y;
    }
}

/// ボールとブロックの衝突判定。当たったブロックは即座に破壊してスコアを加算し、
/// `BrickDestroyed` を発火する（`mark_broken_edges_on_brick_destroyed` observer が拾う）。
/// ブロックは `Transform.scale` を使わず `Brick.size` をメッシュ寸法として直接持つので、
/// 当たり判定の半径もそこから取る（壁/パドル/DeathZone は scale 基準）。
pub fn check_ball_brick_collision(
    mut commands: Commands,
    mut score: ResMut<Score>,
    ball_query: Single<(&mut Velocity, &Transform), With<Ball>>,
    brick_query: Query<(Entity, &Transform, &Brick), Without<Ball>>,
) {
    let (mut ball_velocity, ball_transform) = ball_query.into_inner();

    for (brick_entity, brick_transform, brick) in &brick_query {
        let Some(collision) = check_if_ball_collision_to_another_entity(
            ball_transform,
            &(brick_transform, brick),
        ) else {
            continue;
        };

        commands.trigger(BallCollided);
        commands.entity(brick_entity).despawn();
        score.0 += 1;
        commands.trigger(BrickDestroyed { cell: brick.cell });
        reflect_ball_velocity(&mut ball_velocity, collision);
    }
}

/// ボールと壁（Left/Right/Top）の衝突判定。反射のみで副作用は無い。
pub fn check_ball_wall_collision(
    mut commands: Commands,
    ball_query: Single<(&mut Velocity, &Transform), With<Ball>>,
    wall_query: Query<&Transform, (With<Wall>, Without<Ball>)>,
) {
    let (mut ball_velocity, ball_transform) = ball_query.into_inner();

    for wall_transform in &wall_query {
        let Some(collision) = check_if_ball_collision_to_another_entity(
            ball_transform,
            wall_transform,
        ) else {
            continue;
        };

        commands.trigger(BallCollided);
        reflect_ball_velocity(&mut ball_velocity, collision);
    }
}

/// ボールとパドルの衝突判定。反射のみで副作用は無い（壁と挙動は同じだが、パドルはプレイヤー
/// 操作で毎フレーム動く点が壁と異なるため、`check_ball_wall_collision` とは別の system にしている）。
pub fn check_ball_paddle_collision(
    mut commands: Commands,
    ball_query: Single<(&mut Velocity, &Transform), With<Ball>>,
    paddle_query: Single<&Transform, (With<Paddle>, Without<Ball>)>,
) {
    let (mut ball_velocity, ball_transform) = ball_query.into_inner();
    let paddle_transform = paddle_query.into_inner();

    let Some(collision) = check_if_ball_collision_to_another_entity(ball_transform, paddle_transform) else {
        return;
    };

    commands.trigger(BallCollided);
    reflect_ball_velocity(&mut ball_velocity, collision);
}

/// ボールとデスゾーン（アリーナ下端）の衝突判定。反射はさせず、ライフを1減らす。
/// - 残りライフがあればボールとパドルを初期位置・初速に戻して続行する。
/// - 0 になったら GameOver へ遷移する（ボールはそのフレーム以降、
///   `run_if(in_state(Playing))` により停止する）。
pub fn check_ball_deathzone_collision(
    mut commands: Commands,
    mut lives: ResMut<Lives>,
    mut next_state: ResMut<NextState<GameState>>,
    ball_query: Single<(&mut Velocity, &mut Transform), With<Ball>>,
    deathzone_query: Single<&Transform, (With<DeathZone>, Without<Ball>)>,
    paddle_entity: Single<Entity, With<Paddle>>,
) {
    let (mut ball_velocity, mut ball_transform) = ball_query.into_inner();
    let deathzone_transform = deathzone_query.into_inner();

    let Some(_collision) = check_if_ball_collision_to_another_entity(&ball_transform, deathzone_transform)
    else {
        return;
    };

    commands.trigger(BallCollided);

    lives.0 = lives.0.saturating_sub(1);
    if lives.0 == 0 {
        next_state.set(GameState::GameOver);
        return;
    }

    ball_transform.translation = BALL_STARTING_POSITION;
    ball_velocity.0 = INITIAL_BALL_DIRECTION.normalize() * BALL_SPEED;
    commands.entity(paddle_entity.into_inner()).insert(Transform {
        translation: Vec3::new(0.0, BOTTOM_WALL + GAP_BETWEEN_PADDLE_AND_FLOOR, 0.0),
        scale: PADDLE_SIZE.extend(1.0),
        ..default()
    });
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum Collision {
    Left,
    Right,
    Top,
    Bottom,
}

/// ボールとの当たり判定に使う、ワールド座標上の軸並行矩形（AABB）を返せることを表す。
/// 壁・パドル・デスゾーンは `Transform.scale` を大きさの基準にしているが、ブロックだけは
/// メッシュの実寸である `Brick.size` を直接使う（当たり判定サイズの算出方法が違う）。
/// この差を吸収することで、呼び出し側は `Aabb2d` を自分で組み立てずに済む。
trait BoundingBoxSource {
    fn bounding_box(&self) -> Aabb2d;
}

impl BoundingBoxSource for Transform {
    fn bounding_box(&self) -> Aabb2d {
        Aabb2d::new(self.translation.truncate(), self.scale.truncate() / 2.0)
    }
}

impl BoundingBoxSource for (&Transform, &Brick) {
    fn bounding_box(&self) -> Aabb2d {
        let (transform, brick) = self;
        Aabb2d::new(transform.translation.truncate(), brick.size / 2.0)
    }
}

/// ボール側の当たり判定に使う、ワールド座標上の円（`BoundingCircle`）を返せることを表す。
/// ボールの直径は `Transform.scale`（`Vec2::splat(BALL_DIAMETER)`, `setup.rs`）にそのまま
/// 入っているので、`BoundingBoxSource` の「`scale` から大きさを取る」考え方と揃えられる
/// （x/y は常に等しいので `scale.x` を直径として使う）。
trait BoundingCircleSource {
    fn bounding_circle(&self) -> BoundingCircle;
}

impl BoundingCircleSource for Transform {
    fn bounding_circle(&self) -> BoundingCircle {
        BoundingCircle::new(self.translation.truncate(), self.scale.x / 2.0)
    }
}

/// `check_ball_deathzone_collision` はボールの `Transform` を書き込みも行うため `&mut Transform`
/// で取得しており、クエリ経由だと変更検知つきの `Mut<Transform>` になる（読み取り専用の他の
/// 呼び出し箇所は素の `&Transform` なのでこの impl は要らない）。フィールドアクセスは自動 deref
/// で `Transform` まで届くので、本体は上の `Transform` 版と同じ書き方で済む。
impl BoundingCircleSource for Mut<'_, Transform> {
    fn bounding_circle(&self) -> BoundingCircle {
        BoundingCircle::new(self.translation.truncate(), self.scale.x / 2.0)
    }
}

// Returns `Some` if `ball` collides with `source`'s bounding box.
// The returned `Collision` is the side of the bounding box that `ball` hit.
fn check_if_ball_collision_to_another_entity<C: BoundingCircleSource, B: BoundingBoxSource>(
    ball: &C,
    source: &B,
) -> Option<Collision> {
    let ball = ball.bounding_circle();
    let bounding_box = source.bounding_box();
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
