# Bevy：`Mut<'_, Transform>` の型不一致と衝突判定モジュールの分割

日付: 2026-07-28

`ball_collision_to_another_entity` のジェネリクス化（[[20260728_collision-bounding-box-generics]]）の
過程で、ボールの `Transform` を書き込みも行う箇所だけ `Mut<'_, Transform>` という型不一致に当たった。
その対処（専用 impl の追加）と、衝突判定コードを `systems.rs` から `systems/collision.rs` という
子モジュールへ切り出したモジュール分割の経緯、そして `main.rs` の `.after(check_ball_brick_collision)`
が実際には 3 段のオブザーバ経由の流れを指しているという話をまとめる。

前提知識:
- `Transform`/`(&Transform, &Brick)` にジェネリック境界となるトレイトを実装した設計 →
  [[20260728_collision-bounding-box-generics]]
- `Mut<'w, T>` のライフタイム構造、および素の `&mut T` と独自ラッパー型とで暗黙 reborrow の
  効き方が違うという基礎知識 → [[20260728_rust-lifetimes-basics]]
- ラッパー Resource の `.0` と、`Deref` 派生に対する rust-analyzer の誤検知（同系統の問題を
  `Mut<'_, Transform>` でも避けた） → [[20260723_deref-newtype-vs-dot-zero]]
- `Single<T>::into_inner()`（`*` を踏まずに中身を取り出す） → [[20260723_bevy-single-into-inner]]
- `main.rs` のモジュール分割方針（依存は一方向、可視性は `pub`/非公開で線引き） →
  [[20260715_main-rs-module-split]]
- `BrickDestroyed` のオブザーバ型イベントとしてのライフサイクル（3 節で使う） →
  [[20260725_brick-destroyed-observer-event-lifecycle]]

---

## 1. `Mut<'_, Transform>` だけ型が合わなかった問題

4 つの `check_ball_*_collision` のうち `check_ball_deathzone_collision` だけがボールの
`Transform` を書き込みも行うため `&mut Transform` で取得しており、クエリ経由だと Bevy 独自の
変更検知つきラッパー `Mut<'_, Transform>` になる（`game_engine/src/systems/collision.rs:115,119`）。
他の 3 箇所は読み取り専用の `&Transform` なので素の参照のまま `BoundingCircleSource` に渡せたが、
ここだけ型が合わずコンパイルエラーになった。これは
[[20260728_rust-lifetimes-basics]] 5 節で確認した「独自ラッパー構造体は素の `&mut T` のような
暗黙 reborrow の対象にならない」という非対称性がそのまま実務で表面化した例。

- 最初は `&*ball_transform` で明示的に deref して解決したが、これは Bevy のラッパー型に対する
  `*` の使用であり、[[20260723_deref-newtype-vs-dot-zero]] や [[20260723_bevy-single-into-inner]]
  で扱った「rust-analyzer が derive マクロ由来の `Deref` を誤検知する系統」の問題を再び持ち込む
  書き方だと判断し、避けたいと判断した。
- 代わりに `Mut<'_, Transform>` 専用の実装を追加し、呼び出し側は `*` を使わず `&ball_transform`
  のまま渡せるようにした:

```rust
// game_engine/src/systems/collision.rs:187-195
/// `check_ball_deathzone_collision` はボールの `Transform` を書き込みも行うため `&mut Transform`
/// で取得しており、クエリ経由だと変更検知つきの `Mut<Transform>` になる（読み取り専用の他の
/// 呼び出し箇所は素の `&Transform` なのでこの impl は要らない）。フィールドアクセスは自動 deref
/// で `Transform` まで届くので、本体は上の `Transform` 版と同じ書き方で済む。
impl BoundingCircleSource for Mut<'_, Transform> {
    fn bounding_circle(&self) -> BoundingCircle {
        BoundingCircle::new(self.translation.truncate(), self.scale.x / 2.0)
    }
}
```

本体（`self.translation.truncate()` / `self.scale.x`）は `Transform` 版と全く同じ書き方で済む。
これは `self: &Mut<'_, Transform>` に対するフィールドアクセスがコンパイラの自動 deref で
`Transform` まで自動的にたどり着くため（`*` を明示する必要がない）。呼び出し側:

```rust
// game_engine/src/systems/collision.rs:119-122
let (mut ball_velocity, mut ball_transform) = ball_query.into_inner();
let deathzone_transform = deathzone_query.into_inner();

let Some(_collision) = ball_collision_to_another_entity(&ball_transform, deathzone_transform)
```

`&ball_transform` は `&Mut<'_, Transform>` そのままで、`*` を一度も踏んでいない。

## 2. モジュール構造のリファクタ（2 段階）

1. まず衝突判定関連（`Collision` 列挙、`BoundingBoxSource`/`BoundingCircleSource` とその実装、
   `ball_collision_to_another_entity`、`reflect_ball_velocity`、4 つの `check_ball_*_collision`）を
   `systems.rs` から `collision.rs` という別ファイルに切り出した。
2. その後「`main` からは `systems.rs` だけを読み、`systems.rs` が `collision` を読むという
   依存関係にしたい」という指摘を受け、`collision.rs` をトップレベル（`main` の子モジュール）
   ではなく `game_engine/src/systems/collision.rs`（`systems` の子モジュール）に配置し直した。

```rust
// game_engine/src/systems.rs:1-10
//! 毎フレーム走るゲームプレイ system（パドル移動・速度適用・スコア更新・ブロックの破れ状態更新）と
//! 衝突音の再生。ボールと他エンティティの当たり判定そのものは子モジュール `collision` に分離し、
//! ここで `pub use` して外（`main`）からは `systems::` 経由でまとめて見えるようにしている
//! （`main` は `systems` だけを読み、`systems` が `collision` を読むという依存の向きにするため）。

mod collision;
pub use collision::{
    check_ball_brick_collision, check_ball_deathzone_collision, check_ball_paddle_collision,
    check_ball_wall_collision,
};
```

`main.rs` 側は `systems::{check_ball_brick_collision, ...}` を import しているだけで、
`collision` という名前は一切出てこない（`game_engine/src/main.rs:36-41`）。依存の向きは
`main → systems → collision` の一方向になり、[[20260715_main-rs-module-split]] で確立した
「一方向で循環なし」の方針を `systems` 配下にもそのまま適用した形。

## 3. `main.rs` の `.after(check_ball_brick_collision)` が指す実際の一連の流れ

`main.rs` の次の 1 行は、コメントだけでは分かりにくいが実際には 3 つの関数を跨ぐ一連の流れを
指している:

```rust
// game_engine/src/main.rs:91-95
// `check_ball_brick_collision` の Commands（ブロック despawn / `BrickDestroyed` トリガー）→
// `mark_broken_edges_on_brick_destroyed`(observer) → `BrokenEdges` 変更 という一連の後に
// 読む必要があるため、`.after` で明示する。Bevy は順序制約のある両者の間に自動で
// 同期点（コマンド適用）を挿入するので、これで同一フレーム内の反映が保証される。
.add_systems(Update, redraw_broken_bricks.after(check_ball_brick_collision))
```

実際の流れは 3 段:

1. `check_ball_brick_collision`（`game_engine/src/systems/collision.rs:44-66`）がブロックを
   `despawn` し、`BrickDestroyed { cell }` を `commands.trigger` で発火する。
2. `mark_broken_edges_on_brick_destroyed`（`game_engine/src/systems.rs:181-198`。
   `main.rs:108` の `.add_observer(mark_broken_edges_on_brick_destroyed)` で登録済みの
   オブザーバ）がそれを受けて、隣接ブロックの `BrokenEdges` を立てる。
3. `redraw_broken_bricks`（`rendering.rs`）が `Changed<BrokenEdges>` を見てメッシュを再構築する。

`main.rs` の `.add_systems` 上のコード表現では ① と ③ の関係（`.after`）しか書けない
（オブザーバは `add_systems` のスケジュールとは別の仕組みで登録されるため、`.after` の対象に
できない）。しかし実際には ② のオブザーバが ① と ③ の間に必ず挟まっており、`.after` の
コメントが「オブザーバ経由の後」という意味であることを明示している。オブザーバ型イベントが
「撃った直後の同期点で即時ディスパッチされる」という性質そのものは
[[20260725_brick-destroyed-observer-event-lifecycle]] で詳しく扱っている。

## 4. まとめ

- 読み取り専用の `&Transform` は素の参照のまま渡せるが、書き込みも行う箇所だけクエリ経由で
  `Mut<'_, Transform>` になる。これは独自ラッパー構造体が素の `&mut T` のような暗黙 reborrow の
  対象にならないためで、`*` を踏む代わりに `Mut<'_, Transform>` 専用の impl を用意して解決した。
- 衝突判定コードは `systems.rs` → `systems/collision.rs` という「`systems` の子モジュール」に
  配置し、`main.rs` からは `collision` という名前が一切見えない一方向の依存にした。
- `main.rs` の `.after(check_ball_brick_collision)` は、実際には間に挟まるオブザーバ
  （`mark_broken_edges_on_brick_destroyed`）を経由した 3 段構成の後続処理を指している。

## 5. 関連ファイル

- `game_engine/src/systems/collision.rs` — `Mut<'_, Transform>` 用 impl、4 つの
  `check_ball_*_collision`
- `game_engine/src/systems.rs` — `mod collision;` / `pub use` / `mark_broken_edges_on_brick_destroyed`
- `game_engine/src/main.rs` — import と `.after`/`.add_observer` の並び
