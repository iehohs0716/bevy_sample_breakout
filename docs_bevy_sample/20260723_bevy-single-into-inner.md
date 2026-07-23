# Bevy：`Single<T>::into_inner()` とは何か（`*` を踏まず中身を取り出す）

日付: 2026-07-23

`update_scoreboard` / `update_lives` などで `score_root.into_inner()` という呼び出しを使っている。
この `into_inner()` が何者で、なぜ `*score_root`（デリファレンス）ではなくこちらを使うのかを整理する。

前提知識:
- タプル構造体と `.0`（ラッパーの中身の触り方）→ [[20260723_rust-tuple-struct-and-dot-zero]]
- `**` をやめて `.0` に統一した理由（同系統の rust-analyzer 誤検知回避）→ [[20260723_deref-newtype-vs-dot-zero]]
- `Single` を実際に使っているボールの処理 → [[20260723_ball-lifecycle]]

---

## 1. 対象のコード

`game_engine/src/systems.rs` のスコア／ライフ表示更新:

```rust
// game_engine/src/systems.rs:54
pub fn update_scoreboard(
    score: Res<Score>,
    score_root: Single<Entity, (With<ScoreboardUi>, With<Text>)>,
    mut writer: TextUiWriter,
) {
    // `*score_root` でも動くが、rust-analyzer が Single の Deref を解決できず E0614 を誤検知する。
    // `into_inner()` で中身の Entity を取り出せば * を踏まないので誤検知しない（rustc は両方通る）。
    *writer.text(score_root.into_inner(), 1) = score.to_string();
}

// game_engine/src/systems.rs:64
pub fn update_lives(
    lives: Res<Lives>,
    lives_root: Single<Entity, (With<LivesUi>, With<Text>)>,
    mut writer: TextUiWriter,
) {
    *writer.text(lives_root.into_inner(), 1) = lives.to_string();
}
```

`score_root` の型は `Single<Entity, (With<ScoreboardUi>, With<Text>)>`。
「`ScoreboardUi` かつ `Text` を持つエンティティがちょうど 1 個ある」ことを保証する
システムパラメータで、その中身は `Entity` 1 個。`writer.text(...)` は `Entity` を引数に取るので、
`Single<Entity>` という「箱」から中の `Entity` を取り出して渡す必要がある。それが `into_inner()`。

## 2. `into_inner()` とは何か

### プロジェクト側で実装したものではない

`into_inner()` は **Bevy が `Single<T>` に対して用意しているメソッド**。このリポジトリで定義した
関数ではなく、Bevy ライブラリ側で実装済みのものを呼んでいるだけ。関係としては
`String::len()` を呼ぶのと同じで、「呼び出しているだけ・実装は借り物」。

### トレイトの標準メソッドではなく、Rust 全体の命名慣習

`into_inner` は特定のトレイトが定義する標準メソッドではない。**Rust の標準ライブラリ全体で
共有されている命名慣習**であり、多くの「ラッパー型」がそれぞれ自前の `impl` で同名メソッドを
定義している。例:

- `Mutex<T>::into_inner()` → `T`
- `RefCell<T>::into_inner()` → `T`
- `Cursor<T>::into_inner()` → `T`
- `BufWriter<W>::into_inner()` → `W`

Bevy の `Single<T>` もこの慣習に乗って、自分の `impl` に `into_inner()` を持っている。

### 名前の意味

- `into_` … `self` を **消費（move）** することを表す接頭辞。呼んだ後、元のラッパー変数は使えなくなる。
- `_inner` … ラッパーの **中身** を指す。

つまり `into_inner()` は「ラッパーを消費して、中の本体を取り出して返す」メソッド。

### 動作

`Single<Entity>` という箱を消費して、中に入っている `Entity` 本体を取り出して返すだけ。
複雑な計算や副作用は無い。

```rust
let entity: Entity = score_root.into_inner(); // 箱を捨てて中身の Entity を得る
```

## 3. なぜ `*score_root`（Deref）ではなく `into_inner()` か

`Single<T>` は `Deref` を実装しているので、中身を取り出す方法は 2 つある。
**どちらも rustc ではコンパイルが通る**。差は完全にツール（rust-analyzer）都合。

| 書き方 | rustc | rust-analyzer |
|---|---|---|
| `*score_root`（Deref 演算子 `*`） | 通る | `Single` の `Deref` を解決できず **E0614 を誤検知**（エディタに赤線） |
| `score_root.into_inner()`（メソッド呼び出し） | 通る | 誤検知しない（`*` を踏まないため） |

`*score_root` はデリファレンス演算子 `*` を経由する。rust-analyzer は Bevy 側の `Single` の
`Deref` 実装をうまく解決できず、`E0614: type cannot be dereferenced` を誤って出す。
一方 `into_inner()` は演算子 `*` を一切踏まない **通常のメソッド呼び出し** なので、この誤検知が
起きない。`cargo check` は通るのにエディタだけ赤くなる、という状態を避けられる。

これは `**` を `.0` に置き換えた [[20260723_deref-newtype-vs-dot-zero]] と **同系統の
rust-analyzer 誤検知回避テク**（「`*` 経由を避けて、言語標準の機能／通常のメソッドで書く」）。

## 4. タプルを返す `Single` での用途（destructuring）

`Single<(&mut A, &mut B)>` のように **複数コンポーネントをまとめて 1 エンティティから取る** 場合、
`into_inner()` はタプルを返すので、そのまま分解束縛（destructuring）できるのが定石。
この用途は誤検知回避というより **所有権ごとタプルを受け取って個別変数に分けたい** ため。

```rust
// game_engine/src/systems.rs:106（reset_game）
ball: Single<(&mut Transform, &mut Velocity), With<Ball>>,
// ...
let (mut transform, mut velocity) = ball.into_inner();

// game_engine/src/systems.rs:137（check_for_collisions）
ball_query: Single<(&mut Velocity, &mut Transform), With<Ball>>,
// ...
let (mut ball_velocity, mut ball_transform) = ball_query.into_inner();
```

`*ball` では `(&mut Transform, &mut Velocity)` というタプル参照になり分解束縛しづらいが、
`into_inner()` なら所有権ごとタプルを受け取って、そのまま `let (a, b) = ...` に流せる。

## 5. まとめ（このリポジトリでの書き方）

- `Single<T>` から中身を取り出すときは `into_inner()` を使う。`*single` は避ける。
- 理由は 2 つ:
  1. `*` を踏まないので rust-analyzer の E0614 誤検知が出ない（rustc は両方通るが、エディタが赤くなるのを防ぐ）。
  2. タプル `Single<(&mut A, &mut B)>` では `into_inner()` が分解束縛にそのまま繋がる。
- `into_inner()` は Bevy 側の実装を呼んでいるだけ。`into_`（self を消費）＋ `_inner`（中身）という
  Rust 全体の命名慣習に沿った、ラッパーから本体を取り出すメソッド。
