# Bevy：B0001（クエリ借用競合 panic）と `Without<Ball>` による回避

日付: 2026-07-23

`check_for_collisions` の `collider_query` に付いている `Without<Ball>` は、動かした結果を変える
ためのものではなく、**Bevy の B0001 という起動時 panic を止めるためだけ**に存在する。
「実際にはボールは Collider を持たないのに、なぜこの注釈が要るのか」でつまずきやすいので、
B0001 の仕組みと `Without<T>` の役割を整理する。

前提知識:
- このシステム全体の読み方 → [[20260723_check-for-collisions-system]]
- `Single<T>` の `into_inner()` → [[20260723_bevy-single-into-inner]]
- 役割ごとに型を分ける設計（Ball / Collider / DeathZone） → [[20260723_bevy-init-vs-restart-state]]

---

## 1. 症状（実際の panic メッセージ）

`Without<Ball>` を外して実行すると、システム初期化時に次のような panic が出る:

```
error[B0001]: Query<..., (With<Collider>)> in system <Enable the debug feature to see the name>
accesses component(s) <Enable the debug feature to see the name> in a way that conflicts with a
previous system parameter. Consider using `Without<T>` to create disjoint Queries or merging
conflicting Queries into a `ParamSet`.
```

システム名やコンポーネント名が `<Enable the debug feature to see the name>` と伏せられるのは、
`bevy_ecs` の **debug feature が無効**だから。debug feature を有効化すればここに実名
（`check_for_collisions` や `Transform` など）が表示され、原因の特定が楽になる。

## 2. B0001 とは

- **1 つのシステム内**で複数のクエリ／引数が、**同じエンティティの同じコンポーネント** を
  競合する形（片方 `&mut`・もう片方 `&`、または両方 `&mut`）で触りうるときに、
  **起動時（システム初期化時）** に出る panic。
- これは Rust の借用ルール（可変借用と不変借用は同時に持てない／可変借用は 1 つだけ）を、
  **システム引数のレベルで守らせる仕組み**。実行中のデータ破壊を防ぐため、Bevy は
  システムを走らせる前に「借用が両立するか」を検査する。

## 3. `check_for_collisions` での具体

```rust
// game_engine/src/systems.rs:129
ball_query: Single<(&mut Velocity, &mut Transform), With<Ball>>,
// game_engine/src/systems.rs:132
collider_query: Query<
    (Entity, &Transform, Option<&Brick>, Option<&DeathZone>),
    (With<Collider>, Without<Ball>),
>,
```

- `ball_query` は `Transform` を **`&mut`（可変）** で触る。フィルタは `With<Ball>`。
- `collider_query` は `Transform` を **`&`（不変）** で触る。フィルタは `With<Collider>`。

もし `Without<Ball>` が無いと、Bevy は「**`With<Ball>` かつ `With<Collider>` のエンティティが
ありうる**」と判断する。そのようなエンティティの `Transform` を、ball_query が `&mut`・
collider_query が `&` で同時に借りることになり、借用競合 → B0001 panic。

## 4. 最重要ポイント：Bevy は実物のエンティティを見ない、型だけで判定する

ここが一番混乱する所。

- Bevy の B0001 検査は、**実際にボールが `Collider` を持っているか**、あるいは
  **`With<Ball>` かつ `With<Collider>` のエンティティが今この瞬間存在するか** を
  **一切数えない**。
- 見るのは **型情報だけ**、つまり各クエリの「アクセス宣言（`&`/`&mut`）」と「フィルタ
  （`With`/`Without`）」だけ。それらから「重なりうるか？」を **保守的に**判定し、
  ありうるなら（実際には起きなくても）panic する。
- したがって「現実にはボールは `Collider` を持たないから大丈夫」は **通用しない**。
  安全性は **型で保証** する必要がある。

（実データ上、ボールは `Collider` を持たないので collider_query には元々入らない。
それでも型の上では「入りうる」と見なされてしまう、というギャップがこの問題の核心。）

## 5. `Without<Ball>` の役割とコメントの意味

`systems.rs:130-131` のコメント:

> ball_query が Transform を `&mut` で触るため、Collider 側の `&Transform` と競合しないよう
> `Without<Ball>` で両クエリを排他にする（ボールは Collider を持たないので実データは変わらない）。

これを 2 つの観点で読む:

| 観点 | `Without<Ball>` の要否 |
|---|---|
| 実行結果（実データ） | **不要**。どうせボールは `Collider` を持たず collider_query に入らないので、付けても取っても回る結果は同じ |
| 型レベル（B0001 検査） | **必須**。Bevy は「実物」を知らないので、これが無いと重なりうると見なして panic |

つまり `Without<Ball>` は「**実物がどうか**」を変える注釈ではなく、
「**型の上で 2 つのクエリが絶対に重ならないことを Bevy に約束する**」注釈。
コメントの「実データは変わらない」は、まさに「結果は変わらないが型のために要る」という意味。

### 別解：`ParamSet`

B0001 の回避策はもう 1 つあり、競合するクエリを **`ParamSet`** に統合して「同時にはどちらか
一方しか触らない」ことを保証する方法もある（panic メッセージにも案内される）。
ただし今回のように **クエリ集合を `Without<T>` で確実に分離できる**なら、`Without<T>` が
最も軽い（記述も実行コストも小さい）解。

## 6. まとめ

- B0001 は「1 システム内で同じコンポーネントを競合する形で借りうる」ときに **起動時** に出る panic。
- 判定は **型（アクセス宣言＋フィルタ）だけ**で行われ、実物のエンティティ構成は見ない。
  だから「現実には競合しない」は根拠にならず、型で分離する必要がある。
- `Without<Ball>` は collider_query を ball_query と型レベルで排他にし、B0001 を止めるための注釈。
  実行結果は変えない。重い場合の代替は `ParamSet` だが、分離できるなら `Without<T>` が最軽量。
- 名前が伏せ字になるのは `bevy_ecs` の debug feature 無効時。有効化すれば実名が出る。
