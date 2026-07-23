# Bevy：`**score` の二重デリファレンスをやめて `.0` に統一した理由

日付: 2026-07-23

ライフ機能の実装中に `**lives = ...` / `**score += 1` という **二重デリファレンス（`**`）** で
ラッパー Resource の中身を書き換えていた箇所を、すべて `.0`（タプルフィールド直接アクセス）に
書き換えた。その理屈と経緯を整理する。

前提知識:
- Resource / `Res` / `ResMut` とは → [[20260716_bevy-resource-res-resmut-basics]]
- タプル構造体と `.0`（`Score(usize)` の中身をなぜ `.0` で触るのか）→ [[20260723_rust-tuple-struct-and-dot-zero]]

---

## 1. 対象のコード

`Score` と `Lives` は「`usize` を 1 個だけ包んだ」ラッパー Resource（newtype / タプル構造体）:

```rust
// game_engine/src/components.rs
#[derive(Resource, Deref, DerefMut)]
pub struct Score(pub usize);   // 中身は .0 に入っている usize

#[derive(Resource, Deref, DerefMut)]
pub struct Lives(pub usize);
```

これに対して、当初はこう書いていた:

```rust
**score += 1;                       // スコアを 1 増やす
**lives = lives.saturating_sub(1);  // ライフを 1 減らす
```

これを次のように直した:

```rust
score.0 += 1;
lives.0 = lives.0.saturating_sub(1);
```

## 2. なぜ `**`（星 2 つ）になっていたのか

`score: ResMut<Score>` に対して `*` を何回付けると何になるかを追う。**デリファレンス（`*`）は
「包みを 1 枚はがす」操作**で、ここでは包みが 2 枚重なっている。

| 書き方 | 型 | 何をはがしたか |
|---|---|---|
| `score`   | `ResMut<Score>` | アクセス券（借りている状態） |
| `*score`  | `Score`         | `ResMut` の包みを 1 枚はがす |
| `**score` | `usize`         | さらに `Score` の包みをはがして中身の数値 |

包みが 2 枚ある理由:

1. **`ResMut<T>` 自体がスマートポインタ**で、`Deref` により `*` すると中身の `T`（= `Score`）になる。
2. **`#[derive(Deref, DerefMut)]`** が `Score` → `usize` の `Deref` を自動生成する。
   単一フィールドの構造体なら「その構造体を `*` するとフィールド（`.0`）になる」実装が付く。

この 2 段重ねのせいで `**` と星が 2 つ必要になる。**ポインタのポインタを参照しているように見えて
気持ち悪い**のはこのため。rustc 上はコンパイルできる（`**score` は `usize` の左辺値になる）が、
読み手には「何段はがしているのか」が直感的に伝わらない。

## 3. `.0` にすると何が起きるか

`score.0` は **タプル構造体のフィールドアクセス**。Rust はフィールドを探すために受け手を
必要なだけ自動でデリファレンスする:

- `ResMut<Score>` に `.0` は無い → `ResMut` の `Deref` で 1 枚はがして `Score` に。
- `Score` には `.0: usize` がある → これがフィールド。

つまり `score.0` は「`ResMut` を 1 枚はがして、`Score` のフィールド `.0` を直に触る」。
`**` と結果は同じ `usize` だが、**何を触っているかがコード上に明示される**（`Score` の中身の `.0`）。

## 4. 一番の実利：rust-analyzer の誤検知（E0614）を避けられる

`.0` に統一した決め手はここ。

`**` は **`Score` に生えた「派生された `Deref`」に依存**する。この `Deref` は
`#[derive(Deref)]` という **プロシージャルマクロ（proc-macro）** が生成するもの。
rust-analyzer はこの derive マクロの展開に失敗することがあり、その場合
**「`Score`/`Lives` は `Deref` を実装していない」と誤認**して、エディタ上に

```
E0614: type `ResMut<'_, Lives>` cannot be dereferenced
```

という赤エラーを出す。**`cargo check` / `cargo build`（＝ rustc 本体）は通る**のに、
エディタだけ赤くなる状態になり、「壊れている」ように見えてしまう。

一方 `.0` は **タプルフィールドという言語標準の機能**で、`Score` の派生 `Deref` を一切必要としない
（使うのは `ResMut` 自身の `Deref` だけ）。マクロ展開の成否に左右されないので、
rust-analyzer でも安定して解決でき、誤検知の E0614 が出ない。

## 5. 方針（このリポジトリでの書き方）

- ラッパー Resource / Component（`Score(usize)`, `Lives(usize)` など）の**中身の読み書きは `.0` を使う**。
- `**`（二重デリファレンス）は使わない。既存コードに `**score += 1` のような書き方が残っていても踏襲しない。
- 理由は 2 つ:
  1. 何を触っているか（`.0` = 内側のフィールド）がコード上で明示され、読みやすい。
  2. rust-analyzer の E0614 誤検知を避けられる（rustc は通るのにエディタだけ赤くなる、を防ぐ）。

（`.0` は [[20260716_bevy-resource-res-resmut-basics]] の例でも既に使われており、本来の書き方に揃えた形。）

## 6. 補足：`Deref` 派生そのものが不要なら外す手もある

今回は `.0` に統一するだけに留めたが、`Score`/`Lives` を `.0` でしか触らないなら
`#[derive(Deref, DerefMut)]` を外して `#[derive(Resource)]` だけにする選択肢もある
（派生 `Deref` に依存するコードが他に無いことが前提）。今回はスコープを広げないため据え置いた。
