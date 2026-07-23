# ボール(ball)のライフサイクル — 生成→更新→（停止）

作成日: 2026-07-23

このプロジェクトは Bevy 製のブロック崩し（Breakout）ゲーム（作業ディレクトリ `game_engine/`）。
本ノートは **ボール(ball)** に着目し、その一生（生成 → 毎フレーム更新 → 終端）をコードに沿って追う。

重要な設計上の前提:

- **ボールは常に 1 個だけ**。`Startup` の `setup` system で無条件に 1 個だけ spawn される。
- **ボールを despawn するコードは存在しない**。ブロックは衝突で消えるが、ボールは最後まで
  world に生き続ける。ゲームの「終端」はボールの消滅ではなく、**状態遷移によってプレイ用
  system が止まり、ボールが凍結（停止）する**ことで表現される。

関連ノート:

- ゲーム状態管理とクリア通知の実装: [[20260717_game-state-and-clear-notification-impl]]
- `add_systems` の第一引数（スケジュールラベル / `OnEnter` 等）の網羅: [[20260717_bevy-add-systems-schedule-labels]]
- Bevy→フロント通知の設計方針: [[20260717_bevy-to-frontend-event-notification]]

---

## 1. 関連する型定義

ボールに関わる Component / Event / Resource / State と定数を先に押さえる。

### マーカー・データ Component

`Ball` はデータを持たないマーカー Component。「このエンティティはボールである」というタグ付けだけを行い、
`With<Ball>` でクエリを絞るために使う。

```rust
// src/components.rs:26
#[derive(Component)]
pub struct Ball;
```

`Velocity` は速度ベクトル。`Deref`/`DerefMut` を derive しているので `velocity.x` のように
中身の `Vec2` に直接アクセスできる。ボールの移動と反射はこの値を読み書きして行う。

```rust
// src/components.rs:28-29
#[derive(Component, Deref, DerefMut)]
pub struct Velocity(pub Vec2);
```

### イベント / リソース

`BallCollided` は「ボールが何かに衝突した」ことを表すイベント。衝突音再生の Observer が購読する。

```rust
// src/components.rs:31-32
#[derive(Event)]
pub struct BallCollided;
```

`CollisionSound` は衝突音アセットのハンドルを保持するリソース。`setup` で挿入され、
衝突時に再生される。

```rust
// src/components.rs:37-38
#[derive(Resource, Deref)]
pub struct CollisionSound(pub Handle<AudioSource>);
```

### ゲーム状態 `GameState`

ボールの「終端」に直結する enum。`Playing` が初期状態で、ゲームプレイ system はこの状態でのみ動く。
`Cleared` / `GameOver` に入ると Playing 専用 system が止まり、結果としてボールが凍結する。

```rust
// src/components.rs:14-20
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Playing,
    Cleared,
    GameOver,
}
```

### ボール関連の定数

初期位置・直径・速度・初速方向・色をすべて定数で持つ。

```rust
// src/config.rs:13-17
// We set the z-value of the ball to 1 so it renders on top in the case of overlapping sprites.
pub const BALL_STARTING_POSITION: Vec3 = Vec3::new(0.0, -50.0, 1.0);
pub const BALL_DIAMETER: f32 = 15.;
pub const BALL_SPEED: f32 = 400.0;
pub const INITIAL_BALL_DIRECTION: Vec2 = Vec2::new(0.5, -0.5);
```

```rust
// src/config.rs:50
pub const BALL_COLOR: Color = Color::srgb(1.0, 0.5, 0.5);
```

- z=1 にすることで、重なり時にボールが最前面に描画される。
- 初速は `INITIAL_BALL_DIRECTION` を正規化してから `BALL_SPEED` を掛ける（後述の spawn を参照）。

---

## 2. 生成 (Spawn)

ボールは `Startup` スケジュールの `setup` system の中で、一度だけ spawn される。

```rust
// src/setup.rs:72-80
    // Ball
    commands.spawn((
        Mesh2d(meshes.add(Circle::default())),
        MeshMaterial2d(materials.add(BALL_COLOR)),
        Transform::from_translation(BALL_STARTING_POSITION)
            .with_scale(Vec2::splat(BALL_DIAMETER).extend(1.)),
        Ball,
        Velocity(INITIAL_BALL_DIRECTION.normalize() * BALL_SPEED),
    ));
```

spawn されるコンポーネント一式と初期値:

| コンポーネント | 内容 | 初期値 |
|---|---|---|
| `Mesh2d(Circle::default())` | 円形メッシュ | 半径 0.5 の単位円（`Transform.scale` で拡大） |
| `MeshMaterial2d(BALL_COLOR)` | 描画色 | `srgb(1.0, 0.5, 0.5)`（薄い赤） |
| `Transform` | 位置・スケール | 位置 `(0.0, -50.0, 1.0)`、スケール `(15, 15, 1)`（直径 15） |
| `Ball` | マーカー | — |
| `Velocity` | 速度 | `(0.5, -0.5)` を正規化 ×400 ≒ `(283, -283)`（右下方向へ） |

`setup` は `main.rs` で `Startup` に登録されているため、起動時に 1 回だけ実行される。

```rust
// src/main.rs:57
        .add_systems(Startup, setup)
```

`setup` 関数自体はカメラ・背景・音・パドル・ボール・スコア表示・壁・ブロックをまとめて spawn する
（`src/setup.rs:21` 以降）。ボールに関する部分は上記の 1 ブロックだけであり、
**この spawn を最後にボールが再生成される箇所はコードのどこにも無い**。

---

## 3. 更新 (Update)

ボールに関わる毎フレーム処理は `main.rs` で `Update` に登録される。3 つの system を
`.chain()` で順序固定し、さらに `run_if(in_state(GameState::Playing))` で **プレイ中のみ** 動かす。

```rust
// src/main.rs:60-67
        // ゲームプレイ system はプレイ中（Playing）のみ動かす。クリア後はボールを止める。
        .add_systems(
            Update,
            (apply_velocity, move_paddle, check_for_collisions)
                // `chain`ing systems together runs them in order
                .chain()
                .run_if(in_state(GameState::Playing)),
        )
```

`run_if(in_state(GameState::Playing))` が付いている点が「終端＝停止」設計の要（4章参照）。

### 3-1. 移動 — `apply_velocity`

`Velocity` を持つ全エンティティ（＝ボール）の `Transform` を、速度 × 経過秒で並進させる。

```rust
// src/systems.rs:45-50
pub fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_secs();
        transform.translation.y += velocity.y * time.delta_secs();
    }
}
```

### 3-2. 衝突判定・反射・スコア — `check_for_collisions`

ボール（`Single<..., With<Ball>>`）と、全 `Collider`（壁・パドル・ブロック）を突き合わせる。

```rust
// src/systems.rs:60-111
pub fn check_for_collisions(
    mut commands: Commands,
    mut score: ResMut<Score>,
    ball_query: Single<(&mut Velocity, &Transform), With<Ball>>,
    collider_query: Query<(Entity, &Transform, Option<&Brick>), With<Collider>>,
) {
    let (mut ball_velocity, ball_transform) = ball_query.into_inner();

    for (collider_entity, collider_transform, maybe_brick) in &collider_query {
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

            // Bricks should be despawned and increment the scoreboard on collision
            if maybe_brick.is_some() {
                commands.entity(collider_entity).despawn();
                **score += 1;
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
```

このシステムがボール更新の中心であり、衝突ごとに次の 3 つを行う:

1. **イベント発火** (`src/systems.rs:79`): `commands.trigger(BallCollided)` で衝突を通知。
   これを Observer（後述の `play_collision_sound`）が受けて音を鳴らす。
2. **ブロック消滅とスコア加算** (`src/systems.rs:82-85`): 相手がブロック（`maybe_brick.is_some()`）なら、
   `commands.entity(collider_entity).despawn()` でブロックを消し、`**score += 1`。
   **消えるのはブロックであってボールではない**点に注意。
3. **反射** (`src/systems.rs:88-108`): 衝突面（`Collision::Left/Right/Top/Bottom`）に応じて、
   「めり込む向きに速度が向いているときだけ」`x` または `y` を反転する。この条件付き反転により、
   壁やパドルの内側に入り込んでボールがハマる現象を防いでいる。

### 3-3. 衝突面の判定 — `ball_collision`

ボールの `BoundingCircle` と相手の `Aabb2d` の交差を調べ、衝突していれば
「相手の矩形のどの面に当たったか」を返すヘルパー。

```rust
// src/systems.rs:156-176
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
```

ボール中心から矩形上の最近接点へのオフセットを取り、その x/y いずれの成分が支配的かで面を決める。

### 3-4. 衝突音の再生 — `play_collision_sound`（Observer）

`BallCollided` を購読する Observer。イベントを受け取るたびに、再生後に自動 despawn される
オーディオエンティティを spawn する。

```rust
// src/systems.rs:138-144
pub fn play_collision_sound(
    _collided: On<BallCollided>,
    mut commands: Commands,
    sound: Res<CollisionSound>,
) {
    commands.spawn((AudioPlayer(sound.clone()), PlaybackSettings::DESPAWN));
}
```

Observer は `main.rs` で `add_observer` により登録される。`Update` の `run_if(Playing)` とは独立した
仕組みだが、発火元の `check_for_collisions` が Playing 中しか動かないため、実質プレイ中のみ鳴る。

```rust
// src/main.rs:74
        .add_observer(play_collision_sound)
```

---

## 4. 消滅 / 状態遷移（終端は「凍結」）

**繰り返しになるが、ボールを despawn するコードは存在しない。** ボールは生成後ずっと world に残る。
ゲームの終端は「ボールの消滅」ではなく「状態遷移により Playing 専用 system が止まり、ボールが
凍結する」ことで実現される。

### 4-1. クリア判定 — `check_game_clear`

ブロックが 1 つも無くなったら `GameState::Cleared` へ遷移させる。これも Playing 中のみ動く。

```rust
// src/systems.rs:116-123
pub fn check_game_clear(
    bricks: Query<(), With<Brick>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if bricks.is_empty() {
        next_state.set(GameState::Cleared);
    }
}
```

```rust
// src/main.rs:69
        .add_systems(Update, check_game_clear.run_if(in_state(GameState::Playing)))
```

### 4-2. なぜ「凍結」するのか

`Cleared` に入った瞬間、`in_state(GameState::Playing)` を満たさなくなるため、
`apply_velocity` / `move_paddle` / `check_for_collisions` がすべて実行されなくなる。
`apply_velocity` が止まる＝ボールの `Transform` がもう更新されない＝**ボールがその場で止まる**。
これが `main.rs:60` のコメント「クリア後はボールを止める」の意味である。
ボールのエンティティ自体は残り続けるので、画面上には静止したボールが描画されたままになる。

### 4-3. クリア通知 — `on_game_clear` → JS

`Cleared` への進入時に一度だけ、フロント(JS)へゲームクリアを通知する。

```rust
// src/systems.rs:127-129
pub fn on_game_clear(score: Res<Score>) {
    notify_game_clear(score.0);
}
```

```rust
// src/main.rs:72
        .add_systems(OnEnter(GameState::Cleared), on_game_clear)
```

```rust
// src/notify.rs:14-17
#[cfg(target_arch = "wasm32")]
pub fn notify_game_clear(score: usize) {
    dispatch_event("breakout:gameclear", score);
}
```

`notify_game_clear` は WASM ビルドでのみ `window` に `breakout:gameclear` イベントを dispatch する
（ネイティブビルドでは no-op）。遷移そのものは React 側が担う。詳細は
[[20260717_game-state-and-clear-notification-impl]] を参照。

### 4-4. GameOver は未実装

`GameOver` の器（通知 system と `OnEnter` 登録）は用意済みだが、**`GameState::GameOver` へ遷移する
コードは存在しない**。したがって現状このパスは発火しない。

```rust
// src/systems.rs:134-136
pub fn on_game_over(score: Res<Score>) {
    notify_game_over(score.0);
}
```

```rust
// src/main.rs:73
        .add_systems(OnEnter(GameState::GameOver), on_game_over)
```

`GameOver` への遷移が無いため、ボールが下端（Bottom の壁）に達しても、
`check_for_collisions` の反射ロジック（`src/systems.rs:88-108`）によって単に跳ね返るだけで、
ゲームオーバーにはならない。「ボール落下でゲームオーバー」を実装するなら、下端到達を検知して
`next_state.set(GameState::GameOver)` を呼ぶ system を追加するのが自然な拡張になる。

---

## 5. 全体像（生成 → 更新 → 停止のフロー）

```
[Startup]                                       (一度きり)
  setup (setup.rs:72-80)
    └─ commands.spawn((Mesh2d, MeshMaterial2d, Transform, Ball, Velocity))
         位置(0,-50,1) / 直径15 / 初速 (0.5,-0.5).normalize()*400
                     │
                     ▼
[Update]  run_if(in_state(Playing))  ← Playing の間だけ毎フレーム
  ┌───────────────────────────────────────────────────────────┐
  │ apply_velocity (systems.rs:45-50)                          │
  │   Transform += Velocity * dt   (ボールが動く)              │
  │                     │                                       │
  │ check_for_collisions (systems.rs:60-111)                   │
  │   ball_collision (systems.rs:156-176) で衝突面判定         │
  │     ├─ trigger(BallCollided) (:79) ─▶ play_collision_sound │
  │     │                                  (Observer :138-144) │
  │     ├─ ブロックなら despawn + score++ (:82-85)             │
  │     │   ※消えるのはブロック。ボールは消えない             │
  │     └─ 反射: reflect_x/reflect_y で速度反転 (:88-108)      │
  └───────────────────────────────────────────────────────────┘
                     │
[Update]  check_game_clear (systems.rs:116-123)  run_if(Playing)
  ブロックが 0 個 → next_state.set(Cleared)
                     │
                     ▼
[OnEnter(Cleared)] on_game_clear (systems.rs:127-129)
  notify_game_clear(score) ─▶ (WASM) window "breakout:gameclear" ─▶ React
                     │
                     ▼
  Playing でなくなる → apply_velocity 等が停止
  ★ボールは despawn されず、その場で「凍結（静止）」する★

[GameOver] は遷移コードが無く未到達（器のみ: systems.rs:134-136 / main.rs:73）。
下端に当たってもボールは反射するだけ。
```

### まとめ

- ボールは `Startup` で **1 個だけ** 生成され、以後 **despawn されない**。
- `Update`（Playing 中）で移動・衝突・反射・スコア・音を処理する。消えるのはブロック。
- 終端は状態遷移（`Cleared`）による **Playing 専用 system の停止＝ボールの凍結** で表現される。
- `GameOver` は器だけあり未実装。ボール落下によるゲームオーバーは今後の拡張余地。
