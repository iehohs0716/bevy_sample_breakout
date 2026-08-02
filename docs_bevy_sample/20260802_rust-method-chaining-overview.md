# Rust における「メソッドチェーン」全般

日付: 2026-08-02

一次情報（Rust公式ドキュメント）を確認した上でまとめる。個別の1行・1関数レベルの深掘りは
[[20260802_option-combinator-chain]] / [[20260729_option-take-unwrap-or-else]] /
[[20260731_question-mark-operator-early-return]] に譲り、ここでは「そもそもメソッドチェーンとは
何か」「Rustにどんな種類があるか」「何に注意すべきか」を俯瞰する。

## 1. メソッドチェーンとは何か

```rust
a.method1().method2().method3()
```

という、メソッド呼び出しを`.`でつなげて書くスタイルのこと。特別な構文ではなく、単に
「`method1()`の返り値に対して`method2()`を呼び、その返り値に対して`method3()`を呼ぶ」を
1行で書いているだけ。中間変数に入れずに書くと次のようになる。

```rust
let tmp1 = a.method1();
let tmp2 = tmp1.method2();
let tmp3 = tmp2.method3();
```

チェーンできるかどうかは「そのメソッドが、次にチェーンしたい型の値を返すかどうか」だけで決まる。

## 2. なぜRustでこの書き方が多用されるか

- **所有権・借用ルールと相性が良い**: 中間変数を作ると、それが値を消費する型なら`mut`や
  再代入が必要になりがちで、逆にチェーンで書くと一時値がそのまま次へ流れるだけで済む。
- **`null`や例外の代わりに`Option`/`Result`を使う文化**: 多くの言語なら`if (x != null)`や
  `try/catch`で書く分岐を、Rustでは`Option`/`Result`の「メソッドを呼べば分岐込みで次に進む」
  コンビネータ（後述）で表現する。分岐のたびに`if`を書かずに済む。
- **`Iterator`の遅延評価**: `map`や`filter`などのアダプタは呼んだ時点では何も実行されず、
  最後に`collect`等の終端メソッドを呼んだ瞬間にまとめて1パスで実行される。中間の`Vec`を
  作らずに処理を合成できるので、チェーンで書くこと自体が最適化にもなる。

## 3. 3大ファミリー

Rustで「メソッドチェーン」と言うとき、実質的には次の3つのどれかを指していることが多い。
見た目は似ているが、それぞれ前提にしている型・意味論が違う。

### 3.1 `Option<T>` / `Result<T, E>` のコンビネータ

「値があるかもしれない（`Option`）」「成功/失敗するかもしれない（`Result`）」を表す型に対して、
中身を変換したり、無かった場合の代替を用意したりするメソッド群。代表的なものだけ挙げる
（全量は[[20260802_option-combinator-chain]]や公式ドキュメントを参照）。

| 種類 | 例 | 意味 |
|---|---|---|
| 変換 | `map(f)` | 中身があれば`f`で変換、無ければそのまま`None`/`Err` |
| 変換して合流 | `and_then(f)`（`Result`は同名） | `f`自体が`Option`/`Result`を返すときに、二重に包まれるのを防ぐ |
| 論理結合 | `and` / `or` / `or_else` | 「両方要る」「どちらかでよい」を表現 |
| 複数を待ち合わせ | `zip` | `Option`同士を、両方`Some`のときだけタプルにまとめる |
| 取り出し | `unwrap_or(x)` / `unwrap_or_else(\|\| x)` | 無かったときの代替値（即時評価 or 遅延評価） |
| 借用 | `as_ref()` / `as_mut()` | 中身を消費せず参照だけ覗く（元の変数を後で使いたいときに必須） |
| 書き換え | `take()` / `replace(x)` | `&mut`経由で中身を抜き取る/差し替える |
| 平坦化 | `flatten()` | `Option<Option<T>>` → `Option<T>` |
| 相互変換 | `ok_or(e)` / `Result::ok()` | `Option`⇔`Result`の変換 |

このプロジェクトでの実例:

```rust
// injection.rs: Result を Option に変換してから ? で早期リターン
let config = js_sys::Reflect::get(&window, &JsValue::from_str("__BREAKOUT_CONFIG__")).ok()?;

// injection.rs: Option 同士を filter でさらに絞り込む
js_sys::Reflect::get(config, &JsValue::from_str("cellSize"))
    .ok()
    .filter(|v| !v.is_undefined() && !v.is_null())
    .and_then(|cell| { ... })
```

### 3.2 `Iterator` のアダプタ

「複数の要素の並び」を表す`Iterator`に対して、変換・絞り込み・合成を行うメソッド群。
最大の特徴は**遅延評価**: `map`や`filter`などの「アダプタ」は呼んだ時点では何も実行されず、
新しいIteratorを返すだけ。実際に要素が1つずつ流れて処理されるのは、`collect`や`for_each`
などの**終端メソッド**を呼んだとき（または`for`文で回したとき）。

| 種類 | 代表例 | いつ実行されるか |
|---|---|---|
| アダプタ（遅延） | `map` / `filter` / `zip` / `chain` / `flat_map` / `take` / `skip` / `enumerate` | 終端メソッドが呼ばれるまで何もしない |
| 終端（即時実行） | `collect` / `fold` / `sum` / `count` / `for_each` / `find` / `any` / `all` | 呼んだ瞬間にチェーン全体が1回走る |

```rust
let result: Vec<_> = (0..1_000_000)
    .map(expensive_fn)  // まだ何も計算されない
    .take(5)             // まだ何も計算されない
    .collect();           // ここで初めて expensive_fn が5回だけ呼ばれる
```

このプロジェクトでの実例（`injection.rs::diff_brick_layout`）:

```rust
for (position, cell) in candidate_positions.into_iter().zip(candidate_cells) {
    ...
}
```

`candidate_positions.into_iter()`で`Vec<Vec2>`を消費するIteratorにし、`.zip(candidate_cells)`
（`Iterator::zip`）で`candidate_cells: Vec<BrickCell>`と1個ずつペアにしている。`for`文自体が
このIteratorの終端（消費者）にあたる。

### 3.3 コンシューミングビルダー（Builder パターン）

`self`を消費して、設定を1つ反映した`Self`を返すメソッド群。Rust API Guidelines でも
[C-BUILDER](https://rust-lang.github.io/api-guidelines/type-safety.html)として推奨されている定番パターン。

```rust
pub fn method(mut self, param: T) -> Self {
    self.field = param;
    self  // 自分自身を返すので次のメソッドをチェーンできる
}
```

このプロジェクトでの実例（`systems/setup.rs`）:

```rust
Transform::from_translation(BALL_STARTING_POSITION)
    .with_scale(Vec2::splat(BALL_DIAMETER).extend(1.)),
```

`Transform::from_translation(...)`が`Transform`を返し、`.with_scale(...)`はその`Transform`を
消費して`scale`だけ書き換えた新しい`Transform`を返す。Bevyの`Transform`/`Sprite`等の初期化で
頻出する書き方。

## 4. 落とし穴: 同じメソッド名でもトレイトが違えば別物

`Option`にも`Iterator`にも`zip`というメソッドがあるが、**前提にしている「複数」の意味が違う**。

| | `Option::zip` | `Iterator::zip` |
|---|---|---|
| 対象 | 値が1個あるかないか | 値の並び（0〜N個） |
| 動作 | 両方`Some`なら`Some((a, b))`、片方でも`None`なら`None` | 先頭から1個ずつペアにする。短い方に合わせて打ち切り |
| 今回の実例 | `systems/setup.rs`の`brick_pixels.zip(background_pixels)`（[[20260802_option-combinator-chain]]） | `injection.rs`の`candidate_positions.into_iter().zip(candidate_cells)` |

`map`も`Option`/`Result`/`Iterator`のいずれにもあるが、こちらは「中身を変換する」という
骨格が共通なので混乱しにくい。`zip`のように「複数のものを待ち合わせる」系のメソッドは、
「単一の値の有無」の話なのか「要素列」の話なのかで意味が変わるので要注意。

## 5. チェーンを続けられるかは「`self`を消費するか借用か」で決まる

メソッドのレシーバが

- `self`（値渡し）→ **呼んだ元の変数は消費されて使えなくなる**
- `&self` / `&mut self`（借用）→ 元の変数はそのまま残る

のどちらかで、チェーンの後に元の変数を使えるかが変わる。今回の会話で出てきた
`Option::as_ref(&self) -> Option<&T>`は、まさに「この後の`and_then`は`self`を消費するので、
先に`.as_ref()`で借用に変えておかないと元の変数（`brick_image`）が使えなくなる」という理由で
挟んでいた（詳細は[[20260802_option-combinator-chain]]）。

## 6. このプロジェクトでの実例まとめ

| 箇所 | コード | ファミリー |
|---|---|---|
| `injection.rs::injected_brick_layout`等 | `.ok()?` / `.ok().filter(...).and_then(...)` | Option/Result コンビネータ |
| `systems/setup.rs`（ブロック配置解決） | `background_was_overridden.then(\|\| ...).flatten()` | Option コンビネータ |
| `injection.rs::diff_brick_layout` | `candidate_positions.into_iter().zip(candidate_cells)` | Iterator アダプタ |
| `systems/setup.rs`（ボール生成） | `Transform::from_translation(...).with_scale(...)` | コンシューミングビルダー |

## 7. 関連ドキュメント

- [[20260802_option-combinator-chain]] — `bool::then` / `as_ref` / `and_then` / `zip` / `flatten` の1行ずつの詳細解説
- [[20260729_option-take-unwrap-or-else]] — `Option::take` と `unwrap_or_else` の定番コンボ
- [[20260731_question-mark-operator-early-return]] — `?` 演算子との関係
- [[20260725_rust-match-and-pattern-binding]] — `match`によるパターン分解

## 8. 参考（一次情報）

- [`std::option::Option`](https://doc.rust-lang.org/std/option/enum.Option.html)
- [`std::result::Result`](https://doc.rust-lang.org/std/result/enum.Result.html)
- [`std::iter::Iterator`](https://doc.rust-lang.org/std/iter/trait.Iterator.html)
- [Rust API Guidelines — Type safety（C-BUILDER）](https://rust-lang.github.io/api-guidelines/type-safety.html)
