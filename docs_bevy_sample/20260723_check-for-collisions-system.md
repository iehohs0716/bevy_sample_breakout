# Bevy：衝突判定システム `check_for_collisions` の読み方

日付: 2026-07-23

`game_engine/src/systems.rs` の `check_for_collisions` は「ボールが何かにぶつかったか」を毎フレーム
判定し、ぶつかった相手の役割（ブロック／下端／壁・バー）に応じて処理を変えるシステム。
クエリの型・`for` ループの回り方・`Transform` の意味・役割ごとの分岐でつまずきやすいので、
1 つずつほどく。

前提知識:
- `Single<T>` からの `into_inner()`（`ball_query.into_inner()`）→ [[20260723_bevy-single-into-inner]]
- ボールの生成・更新・停止の全体像 → [[20260723_ball-lifecycle]]
- ラッパー Resource の `.0`（`score.0` / `lives.0`）→ [[20260723_deref-newtype-vs-dot-zero]]
- ゲーム状態遷移（GameOver など）→ [[20260723_bevy-init-vs-restart-state]]

---

## 1. `collider_query` の型と意味

```rust
// game_engine/src/systems.rs:129
ball_query: Single<(&mut Velocity, &mut Transform), With<Ball>>,
// ball_query が Transform を `&mut` で触るため、Collider 側の `&Transform` と競合しないよう
// `Without<Ball>` で両クエリを排他にする（ボールは Collider を持たないので実データは変わらない）。
collider_query: Query<
    (Entity, &Transform, Option<&Brick>, Option<&DeathZone>),
    (With<Collider>, Without<Ball>),
>,
```

`collider_query` は「`Collider` を持つエンティティ全部」を対象にしたクエリ。
`Collider` を持つのは次の 4 種類（`setup.rs` で spawn される）:

| エンティティ | Collider | 備考 |
|---|---|---|
| Brick（ブロック） | あり | `Brick` コンポーネントを持つ |
| Paddle（バー） | あり | `game_engine/src/setup.rs:69` |
| Wall（壁：左・右・上） | あり | `#[require(Sprite, Transform, Collider)]`（`components.rs:55`） |
| DeathZone（下端） | あり | `DeathZone::new()` が `Collider` を返す（`components.rs:125`）。`DeathZone` コンポーネントを持つ |

**Ball は `Collider` を持たない**（`setup.rs:75` の spawn は `Ball, Velocity` などだけ）。
ボールは衝突を「する側（＝ぶつけに行く主体）」であって「される側」ではないので `collider_query`
には最初から入らない。さらに `Without<Ball>` でも明示的に除外している（理由は 5 節）。

### タプルは「1 エンティティぶんの情報セット」

`(Entity, &Transform, Option<&Brick>, Option<&DeathZone>)` の 4 要素は、**どれも「その 1 個の
エンティティに紐づく情報」**。`Entity` だけが特別なわけではない。

| 要素 | 何を指すか |
|---|---|
| `Entity` | そのエンティティの ID（後で `despawn` の対象指定に使う） |
| `&Transform` | そのエンティティ 1 個の位置・大きさ |
| `Option<&Brick>` | そのエンティティが `Brick` を持てば `Some`、なければ `None` |
| `Option<&DeathZone>` | そのエンティティが `DeathZone` を持てば `Some`、なければ `None` |

`Brick` / `DeathZone` を `Option` にしているのは、`Collider` を持つ全種類を 1 本のクエリで回しつつ、
「このエンティティは Brick か？ DeathZone か？」を `Some`/`None` で判別するため。

### `for` ループは 1 周ごとに別のエンティティ

```rust
for (collider_entity, collider_transform, maybe_brick, maybe_death) in &collider_query {
```

`for` は **1 周につき 1 エンティティ** を取り出す。つまり `collider_transform` は「全 Collider の
Transform の集合」ではなく、**その周に回ってきたエンティティ 1 個ぶんの位置・大きさ**。
次の周では別のエンティティの Transform になる。ここを「まとめて全部入っている」と誤解しやすい。

## 2. `Transform` とは

`Transform` は **Bevy 標準のコンポーネント**で、各エンティティに 1 個ずつ紐づく。主に:

- `translation`（位置。`Vec3`）
- `rotation`（回転）
- `scale`（大きさ。`Vec3`）

`Entity` と `Transform` は役割が違う（`Entity` = そのエンティティの ID、`Transform` = そのエンティティの
位置と大きさ）が、**どちらも「エンティティ 1 個ぶんの情報」**という点は同じ。1 節のタプルは
「同じ 1 個のエンティティについての、ID・位置大きさ・種類フラグをまとめたもの」と読める。

## 3. 当たり判定の中身

```rust
// game_engine/src/systems.rs:140
let collision = ball_collision(
    BoundingCircle::new(ball_transform.translation.truncate(), BALL_DIAMETER / 2.),
    Aabb2d::new(
        collider_transform.translation.truncate(),
        collider_transform.scale.truncate() / 2.,
    ),
);
```

- ボールは **円**（`BoundingCircle`）として扱う。中心 = ボールの位置、半径 = `BALL_DIAMETER / 2`。
- 相手は **矩形（AABB: 軸並行境界ボックス）**（`Aabb2d`）として扱う。中心 = 相手の位置
  (`translation`)、半サイズ = 大きさの半分 (`scale / 2`)。
- `truncate()` は `Vec3 → Vec2` へ落とす操作（z を捨てて 2D 化）。当たり判定は 2D 平面で行う。

`ball_collision`（同ファイル下部のヘルパー）が円と矩形の交差を調べ、交差していれば
どの面で当たったかを `Some(Collision::Left/Right/Top/Bottom)`、当たっていなければ `None` で返す。

## 4. 衝突後の分岐（役割で処理を変える）

これが CLAUDE.md の設計方針「役割が違うものは別コンポーネントに分け、`Option<&T>` で判別して
振る舞いを変える」を体現している箇所。

```rust
if let Some(collision) = collision {
    commands.trigger(BallCollided);              // 効果音などの observer を起動

    if maybe_death.is_some() {                    // (A) 下端 DeathZone
        lives.0 = lives.0.saturating_sub(1);
        if lives.0 == 0 {
            next_state.set(GameState::GameOver);
        } else {
            ball_transform.translation = BALL_STARTING_POSITION;
            ball_velocity.0 = INITIAL_BALL_DIRECTION.normalize() * BALL_SPEED;
        }
        break;                                    // このフレームの残り衝突判定は打ち切る
    }

    if maybe_brick.is_some() {                     // (B) ブロック Brick
        commands.entity(collider_entity).despawn();
        score.0 += 1;
    }

    // (C) 反射
    match collision {
        Collision::Left => reflect_x = ball_velocity.x > 0.0,
        Collision::Right => reflect_x = ball_velocity.x < 0.0,
        Collision::Top => reflect_y = ball_velocity.y < 0.0,
        Collision::Bottom => reflect_y = ball_velocity.y > 0.0,
    }
    if reflect_x { ball_velocity.x = -ball_velocity.x; }
    if reflect_y { ball_velocity.y = -ball_velocity.y; }
}
```

まず `commands.trigger(BallCollided)` で `BallCollided` イベントを発火する（衝突音などの observer
がこれを拾う）。その後、相手の役割で分岐する。

### (A) DeathZone（下端）に当たった場合

- `lives.0` を `saturating_sub(1)` で 1 減らす（0 未満にならない引き算）。
- 0 になったら `GameState::GameOver` へ遷移。
- まだ残っていればボールを初期位置 `BALL_STARTING_POSITION`・初速 `INITIAL_BALL_DIRECTION` に戻す。
- **反射はしない**。ボールをリセットしたので `break` でこのフレームの残り判定を打ち切る。

### (B) Brick（ブロック）に当たった場合

- `commands.entity(collider_entity).despawn()` でそのブロックを消す。
- `score.0 += 1` でスコア加算。
- **`despawn` は Brick のときだけ**。Paddle・Wall・DeathZone は `Brick` を持たない
  （`maybe_brick` が `None`）ので消えない。ボール・バー・壁が巻き添えで消えることはない。

### (C) 反射（Brick・Wall・Paddle 共通）

DeathZone 以外は速度を反転して跳ね返す。ただし **ボールがその方向へ向かっているときだけ**
反射フラグを立てる（例: `Collision::Left` は `ball_velocity.x > 0.0` のときだけ `reflect_x`）。
これは、すでに離れる向きに動いているボールを二重反射させて **バーや壁にめり込む／貼り付く**
のを防ぐための向きチェック。`reflect_x` / `reflect_y` が立った軸だけ符号を反転する。

## 5. なぜ `Without<Ball>` が要るか（クエリ競合の回避）

同じシステム内で 2 つのクエリが `Transform` に触る:

- `ball_query`（`Single`）… ボールの `Transform` を **`&mut`（可変）** で触る。
- `collider_query`（`Query`）… Collider 側の `Transform` を **`&`（不変）** で触る。

Bevy は「同一エンティティの `Transform` を、可変と不変で同時に借りる」ことを実行時に
禁止する（借用競合）。もし collider_query がボールも拾ってしまうと、同じボールの `Transform` を
`&mut`（ball_query）と `&`（collider_query）で同時に借りることになり競合する。
そこで `collider_query` に **`Without<Ball>`** を付けて両クエリを排他にしている。

実際にはボールは `Collider` を持たないので collider_query には元々入らず、
`Without<Ball>` があってもなくても **実データ上は同じ**。それでも明示しているのは、
Bevy のクエリ競合チェックにコンパイル／実行時点で「排他だ」と伝え、安全に成立させるため
（`systems.rs:130` のコメント参照）。

## 6. まとめ

- `collider_query` は `Collider` を持つ全エンティティ（Brick / Paddle / Wall / DeathZone）を回す。
  Ball は Collider を持たず、`Without<Ball>` でも除外される。
- クエリのタプルは「エンティティ 1 個ぶんの情報セット」。`for` は 1 周 1 エンティティで、
  `collider_transform` はその 1 個の位置・大きさ。
- 判定はボール＝円 × 相手＝AABB。分岐は `Option<&Brick>` / `Option<&DeathZone>` の有無で行い、
  役割ごとに処理（ライフ減／despawn＋加点／反射）を変える。これが「役割ごとに型を分ける」
  設計方針（[[20260723_bevy-init-vs-restart-state]] と同じ思想）の実践。
