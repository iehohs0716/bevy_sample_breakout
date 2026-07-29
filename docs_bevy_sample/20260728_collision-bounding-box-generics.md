# Bevy：衝突判定関数のジェネリクス化（`BoundingBoxSource` / `BoundingCircleSource`）

日付: 2026-07-28

`ball_collision_to_another_entity` を「呼び出し側で `Aabb2d`/`BoundingCircle` を組み立てて渡す」
方式から「`Transform` を持っているものをそのまま渡せる」方式へジェネリクス化した記録。
2 つのトレイト（`BoundingBoxSource`/`BoundingCircleSource`）を新設し、`Transform` だけでなく
`(&Transform, &Brick)` という即席のタプル型にもトレイトを実装することで、呼び出し側の分岐を
関数シグネチャのジェネリック境界に押し込めた。

前提知識:
- 旧衝突判定（`check_for_collisions` 1 関数構成、`Option<&Brick>`/`Option<&DeathZone>` による分岐）
  → [[20260723_check-for-collisions-system]]
- ブロックが `Transform.scale` ではなく `Brick.size` を大きさとして使う理由（動的メッシュ） →
  [[20260725_brick-torn-edge-midpoint-displacement]]
- ジェネリクスの境界（トレイト）とライフタイムパラメータの基礎 →
  [[20260728_rust-lifetimes-basics]]

---

## 1. なぜジェネリクス化したか

旧実装は呼び出し側で `Aabb2d::new(...)` や `BoundingCircle::new(...)` を都度組み立てて
`ball_collision_to_another_entity` へ渡していた。「呼び出し側でボックスを作らず、`Transform` を
持っているものをそのまま渡せるようにしたい」という要望から、2 つのトレイトを新設した
（`game_engine/src/systems/collision.rs`）。

```rust
// game_engine/src/systems/collision.rs:156-171
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
```

```rust
// game_engine/src/systems/collision.rs:177-185
trait BoundingCircleSource {
    fn bounding_circle(&self) -> BoundingCircle;
}

impl BoundingCircleSource for Transform {
    fn bounding_circle(&self) -> BoundingCircle {
        BoundingCircle::new(self.translation.truncate(), self.scale.x / 2.0)
    }
}
```

- `BoundingBoxSource`（矩形側）: `impl for Transform` は壁・パドル・デスゾーン用（`scale` 基準）、
  `impl for (&Transform, &Brick)` はブロック用（`Brick.size` 基準。ブロックは動的メッシュで
  `Transform.scale` を大きさに使わないため。詳細は [[20260725_brick-torn-edge-midpoint-displacement]]）。
- `BoundingCircleSource`（円側）: ボール用。`scale.x` を直径として使う。これは `setup.rs:78` で
  ボールの `Transform` に `Vec2::splat(BALL_DIAMETER)` が `scale` として入っているため
  （`config.rs:15` に `BALL_DIAMETER = 15.`）で成立する前提。

関数本体はこの 2 トレイトをジェネリック境界に取るだけになった:

```rust
// game_engine/src/systems/collision.rs:199-224
fn ball_collision_to_another_entity<C: BoundingCircleSource, B: BoundingBoxSource>(
    ball: &C,
    source: &B,
) -> Option<Collision> {
    let ball = ball.bounding_circle();
    let bounding_box = source.bounding_box();
    if !ball.intersects(&bounding_box) {
        return None;
    }
    // ... 最も近い点との offset から Collision::Left/Right/Top/Bottom を決める
}
```

呼び出し側（`check_ball_brick_collision` など）は `Aabb2d`/`BoundingCircle` を一切知らずに、
`&Transform` や `&(brick_transform, brick)` をそのまま渡せる:

```rust
// game_engine/src/systems/collision.rs:52-58
for (brick_entity, brick_transform, brick) in &brick_query {
    let Some(collision) = ball_collision_to_another_entity(
        ball_transform,
        &(brick_transform, brick),
    ) else {
        continue;
    };
```

## 2. `(&Transform, &Brick)` タプルへ impl した理由

`BoundingBoxSource::bounding_box(&self)` は `&self` 1 つしか受け取れないが、ブロックの
`Aabb2d` を作るには位置（`Transform`）と大きさ（`Brick.size`）の **2 つ** の情報が必要になる。
2 つの参照を 1 つの `&self` にまとめる最小の方法として、タプル `(&Transform, &Brick)` に
トレイトを実装した（`game_engine/src/systems/collision.rs:166-171`）。これは Bevy が用意した
型ではなく、その場で作った「2 個組」であることに注意。

```rust
impl BoundingBoxSource for (&Transform, &Brick) {
    fn bounding_box(&self) -> Aabb2d {
        let (transform, brick) = self;
        Aabb2d::new(transform.translation.truncate(), brick.size / 2.0)
    }
}
```

呼び出し側は `&(brick_transform, brick)` とタプルを組んで渡すだけでよく
（`game_engine/src/systems/collision.rs:55`）、`Aabb2d` 自体の組み立てはトレイト側に隠蔽される。
タプルはユーザー定義の型ではなく標準の複合型だが、自クレートで定義したトレイト
（`BoundingBoxSource`）を実装する対象としては何の問題もない（orphan rule 上、トレイトが自クレート
のものであれば実装対象の型は外部型でもよい）。

## 3. まとめ

- `ball_collision_to_another_entity` は `BoundingCircleSource`/`BoundingBoxSource` という 2 つの
  トレイトをジェネリック境界に取ることで、呼び出し側が `Aabb2d`/`BoundingCircle` を自分で
  組み立てる必要が無くなった。
- 壁・パドル・デスゾーンは `Transform.scale` を大きさとして使えるが、ブロックだけは動的メッシュの
  都合で `Brick.size` が必要になる。この非対称性は「`(&Transform, &Brick)` というタプルにも
  トレイトを実装する」ことで、関数側の分岐を増やさずに吸収した。
- `Mut<'_, Transform>`（書き込み用クエリから取れる型）だけがこの仕組みに素直に乗らなかった問題と、
  衝突判定コードのモジュール分割は [[20260728_mut-transform-and-collision-module-split]] で扱う。

## 4. 関連ファイル

- `game_engine/src/systems/collision.rs` — `BoundingBoxSource` / `BoundingCircleSource` とその実装、
  `ball_collision_to_another_entity`
