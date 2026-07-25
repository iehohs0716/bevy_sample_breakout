# Bevy：`BrickDestroyed`（オブザーバ型イベント）のライフサイクル

日付: 2026-07-25

ブロック破壊時に飛ぶ内部イベント `BrickDestroyed` は、Bevy の **オブザーバ型イベント**
（`trigger` で撃って `On<E>` で受ける即時ディスパッチ型）。`EventReader` で読む
**バッファ型イベント** とは登録・撃ち方・寿命がすべて別物なので、混同しやすい。
登録→発火→ディスパッチ→消費→下流再描画までのライフサイクルを、実コードの行番号付きで追う。

前提知識:
- 破壊イベントを発火する側（衝突判定） → [[20260723_check-for-collisions-system]]
- 破れた辺の再描画とブロックのコンポーネント設計 → [[20260725_brick-torn-edge-midpoint-displacement]]
- Event/observer を使ったフロント通知の類例 → [[20260717_bevy-to-frontend-event-notification]]
- ラッパー Resource の `.0`（`score.0`） → [[20260723_deref-newtype-vs-dot-zero]]

---

## 1. `BrickDestroyed` はオブザーバ型イベント

```rust
// game_engine/src/components.rs:90
#[derive(Event)]
pub struct BrickDestroyed {
    pub cell: BrickCell,
}
```

- `commands.trigger(...)` / `world.trigger(...)` で **撃ち**、`On<BrickDestroyed>` を第 1 引数に
  取るオブザーバ関数で **受ける**。撃った瞬間（正確には次の同期点）に登録済みの全オブザーバへ
  同期的に配達される「即時ディスパッチ型」。
- `#[derive(Event)]` は付けるが、**`add_event` は不要**。バッファを用意する必要がないため。
  受け手を用意するのは `add_observer`（2 節①）だけ。
- 対して `EventReader<E>` で毎フレーム読むのが「バッファ型イベント」。こちらは `add_event::<E>()`
  でバッファを登録し、`EventWriter::send` で書き込む（比較は 4 節）。

`cell: BrickCell` は破壊されたブロックの格子座標。`BrickCell` は `#[derive(Clone, Copy, ...)]`
（`components.rs:61`）の **値型（Copy）** で、イベントは座標を **値でコピーして** 運ぶ。
これが「消えたブロックの座標を、消滅後に隣へ伝える」ことを可能にする鍵（2 節②）。

## 2. ライフサイクル①〜⑤

### ① 登録（起動時 1 回）

```rust
// game_engine/src/main.rs:92
.add_observer(mark_broken_edges_on_brick_destroyed)
```

アプリ構築時に **グローバルオブザーバ** として常設で待ち受け登録する。以降、どこで
`BrickDestroyed` が撃たれてもこの関数が呼ばれる。登録は起動時の 1 回きり。

### ② 発火

```rust
// game_engine/src/systems.rs:194
if let Some(brick) = maybe_brick {
    commands.entity(collider_entity).despawn();
    score.0 += 1;
    commands.trigger(BrickDestroyed { cell: brick.cell });
}
```

`check_for_collisions`（`Update` の `Playing` 中）でボールがブロックに当たった分岐。同じ分岐で
ブロック実体を `despawn` し、`BrickDestroyed` を `trigger` する。**`commands.*` はどちらも遅延
コマンド** なので、この行では即実行されず、次の同期点でまとめて適用される。

**重要（`Entity` ではなく `cell` を載せる理由）**: イベントは `brick.cell` を **値でコピー** して
持つ。同じ分岐でブロック実体を `despawn` するので、後段でオブザーバが走る頃には
そのエンティティは消えている。もし `Entity` を載せていたら「既に消えた実体」への参照になって
しまうが、`cell`（`Copy` の値型）で運べば **実体が消えても座標情報は生き残る**。
「消えたブロックの座標を、消滅後に隣のブロックへ伝える」というこの機能の要求に、値渡しの
`cell` がちょうど合う。

### ③ 同期点でコマンド適用 → 発火 → オブザーバが同期実行

次の同期点で遅延コマンドが適用され、`despawn` の反映と `trigger` の発火が起きる。発火すると
その場で（同じ同期点で）オブザーバが **同期的に** 実行される。

```rust
// game_engine/src/systems.rs:270
pub fn mark_broken_edges_on_brick_destroyed(
    trigger: On<BrickDestroyed>,
    mut bricks: Query<(&Brick, &mut BrokenEdges)>,
) {
    let destroyed = trigger.cell;
    for (brick, mut broken) in &mut bricks {
        let cell = brick.cell;
        if cell.row == destroyed.row + 1 && cell.col == destroyed.col {
            broken.bottom = true; // 真上 → 自分の下辺が破れる
        } else if cell.row == destroyed.row - 1 && cell.col == destroyed.col {
            broken.top = true;    // 真下 → 上辺
        } else if cell.col == destroyed.col + 1 && cell.row == destroyed.row {
            broken.left = true;   // 右隣 → 左辺
        } else if cell.col == destroyed.col - 1 && cell.row == destroyed.row {
            broken.right = true;  // 左隣 → 右辺
        }
    }
}
```

`Query<(&Brick, &mut BrokenEdges)>` で生存中の全ブロックを走査し、`destroyed` の上下左右に
隣接するブロックの「接していた辺」だけ `broken` を立てる。**`despawn` も同じ同期点で反映済み**
なので、消えたブロック自身はこのクエリに現れない（自分自身を対象外にする分岐が要らず好都合）。

### ④ 消費

オブザーバ型イベントは **バッファされない**。撃った 1 回が登録済みの全オブザーバへ配達されたら
終わり。次フレームに残らず、二重に読まれることもない（`EventReader` のように「数フレーム
保持され得る」性質を持たない）。

### ⑤ 下流：`Changed<BrokenEdges>` → 再描画

```rust
// game_engine/src/main.rs:75
// `check_for_collisions` の Commands（ブロック despawn / `BrickDestroyed` トリガー）→
// `mark_broken_edges_on_brick_destroyed`(observer) → `BrokenEdges` 変更 という一連の後に
// 読む必要があるため、`.after` で明示する。Bevy は順序制約のある両者の間に自動で
// 同期点（コマンド適用）を挿入するので、これで同一フレーム内の反映が保証される。
.add_systems(Update, redraw_broken_bricks.after(check_for_collisions))
```

`BrokenEdges` が更新されると `Changed<BrokenEdges>` が立ち、`redraw_broken_bricks` が
そのブロックだけメッシュを再構築する（[[20260725_brick-torn-edge-midpoint-displacement]]）。
`redraw_broken_bricks.after(check_for_collisions)` により Bevy が両者の間に **同期点を自動挿入** し、
「発火 → コマンド適用 → オブザーバ実行 → `BrokenEdges` 変更 → 再描画」が **同一フレーム内**
で順に完了することを保証する（`main.rs:75-79` のコメント）。

## 3. 1 フレーム内の時系列

```
check_for_collisions（Update / Playing）
  └ ブロック衝突分岐:
       commands.entity(brick).despawn()          ← 遅延コマンドをキュー
       score.0 += 1
       commands.trigger(BrickDestroyed{cell})     ← 遅延コマンドをキュー
                    │
              （同期点）  ← Bevy が .after 制約から自動挿入
                    │
       despawn 反映（そのブロックは以後クエリに出ない）
       trigger 発火 → mark_broken_edges_on_brick_destroyed が同期実行
                       隣接4方向のブロックの BrokenEdges を更新
                    │
redraw_broken_bricks（.after(check_for_collisions)）
  └ Changed<BrokenEdges> のブロックだけメッシュ再構築
                    │
              フレーム終了：BrickDestroyed は消費済み（次フレームに残らない）
```

## 4. バッファ型イベントとの比較

| 観点 | オブザーバ型（`BrickDestroyed`） | バッファ型（`EventReader`） |
|---|---|---|
| 撃ち方 | `commands.trigger(e)` / `world.trigger(e)` | `EventWriter::<E>::send(e)` |
| 受け方 | `On<E>` を引数に取るオブザーバ関数 | システム内で `EventReader<E>` を読む |
| タイミング | 同期点で即ディスパッチ（その場で全オブザーバ実行） | バッファに溜め、読む側のシステム実行時にまとめて読む |
| 寿命 | 1 回配達で終わり（残らない） | 数フレーム保持され得る（既定で数フレーム後に破棄） |
| 登録 | `add_observer(...)`（`add_event` 不要） | `add_event::<E>()` でバッファ登録 |

`BrickDestroyed` は「破壊された瞬間に隣へ即座に一度だけ伝えたい」用途なので、バッファに溜めて
後から読むより、その場で同期実行されるオブザーバ型が素直に合う。

## 5. テストでの発火

```rust
// game_engine/src/systems.rs:353
world.add_observer(mark_broken_edges_on_brick_destroyed);
// ... 上下左右・非隣接・斜めのブロックを spawn ...
// game_engine/src/systems.rs:362
world.trigger(BrickDestroyed { cell: cell(0, 0) });
```

テストは `World` に直接 `world.trigger(...)` する。`commands.trigger` と違い **コマンド遅延が無く、
その場で即発火** する。オブザーバを `world.add_observer(...)` で登録しておけば、`Update` の
スケジュールや同期点を回さずに、発火 → オブザーバ実行 → 各ブロックの `BrokenEdges` を
`world.get::<BrokenEdges>(entity)` で検証、という流れを直接テストできる
（`marks_only_the_edge_facing_the_destroyed_neighbor`, `systems.rs:351`）。

## 6. まとめ

- `BrickDestroyed` はオブザーバ型イベント。`trigger` で撃ち `On<E>` で受ける即時ディスパッチ型で、
  `#[derive(Event)]` は要るが `add_event` は不要、受け手は `add_observer` で常設登録する。
- イベントは `cell`（`Copy` の値型）を値で運ぶので、同じ分岐でブロックを `despawn` しても座標は
  生き残り、消滅後に隣へ伝えられる。`Entity` を載せないのはこのため。
- `.after(check_for_collisions)` により、発火→コマンド適用→オブザーバ実行→`BrokenEdges` 変更→
  `redraw_broken_bricks` の再描画が同一フレーム内で順に完了する。
- 撃った 1 回で配達完了、バッファに残らない。テストは `world.trigger` で同期点を待たず検証できる。
