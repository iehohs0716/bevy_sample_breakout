# `Option` のメソッドチェーン（`bool::then` / `and_then` / `zip` / `flatten`）

日付: 2026-08-02

`systems/setup.rs` のブロック配置解決（画像差分の自動配置が発火するかどうかの判定）で
書いたこの1ブロックの意味を整理する。

```rust
let diffed = background_was_overridden
    .then(|| {
        brick_image
            .as_ref()
            .and_then(|(handle, _)| images.get(handle))
            .zip(images.get(&background_handle))
            .and_then(|(brick_img, background_img)| {
                diff_brick_layout(background_img, brick_img, paddle_y, cell_size)
            })
    })
    .flatten();
```

背景は [[20260802_brick-diff-auto-layout]] を参照。ここではこの1ブロックで使われている
`Option` 系メソッドだけを1つずつ切り出して説明する。

## 0. 一言で言うと

「**背景画像・ブロック画像が両方ちゃんと揃っているときだけ `diff_brick_layout` を呼び、
どれか1つでも欠けていたら黙って `None` にする**」という判定を、`if` を積み重ねずに
1本の式で書いたもの。`if`や`match`を連ねる代わりに、「中身があるときだけ次に進む」
メソッドを数珠つなぎにしている。

## 1. `bool::then` — `if` の代わりに `Option` を作る

```rust
background_was_overridden.then(|| { ... })
```

次と同じ意味（`false` のときクロージャの中身は実行されない＝遅延評価）。

```rust
if background_was_overridden {
    Some({ ... })
} else {
    None
}
```

`bool` から `Option` を作る変換だと考えるとよい。似た仲間に `then_some(x)` があるが、
こちらは `x` を条件に関わらず先に評価してしまう（`unwrap_or` と `unwrap_or_else` の
関係と同じ）ので、クロージャの中身にコストがあるときは `then` を使う。

## 2. `Option::as_ref` — 中身を借りるだけ（`move` を避ける）

```rust
brick_image.as_ref()
```

`brick_image` の型は `Option<(Handle<Image>, Vec2)>`。ここで中身を取り出して使って
しまうと `brick_image` 自体が消費され、後ろの `spawn_brick` 呼び出し
（`brick_image.clone()` する箇所）で使えなくなる。`.as_ref()` は「中身を移動せず、
参照だけ覗く」変換で、`Option<T>` を `Option<&T>` に変える。

## 3. `Option::and_then` — 中身があるときだけ次の `Option` を作る

```rust
.and_then(|(handle, _)| images.get(handle))
```

`and_then` は「中身があるときだけ、その中身を使って次の `Option` を作る。中身が無ければ
そのまま `None`」という処理。JS の `?.`（オプショナルチェーン）に近い。

### なぜ `map` ではなく `and_then` なのか（数字で確認する）

一旦このコードから離れて、単純な数字で考える。

```rust
let x: Option<i32> = Some(4);

fn half(n: i32) -> Option<i32> {
    if n % 2 == 0 { Some(n / 2) } else { None }
}
```

`half`は「渡した数を2で割る。奇数なら`None`」という、**それ自体が`Option`を返す関数**。

（あえて`if`/`else`で書いている。中身に`map`/`filter`のような`Option`のメソッドを
使ってしまうと、この後の説明に出てくる「外側の`map`」と紛らわしくなるため。
`half`をもっと関数型っぽく書く話は9節の余談を参照。）

**`.map(half)`で呼ぶと:**

```rust
let a = x.map(half);
```

`map`は「中身があれば関数に渡し、**その関数の返り値をそのまま`Some`で包む**」動きをする。
`half(4)`は`Some(2)`を返すので、それをもう一度`Some`で包んでしまい、

```
a = Some(Some(2))   ← Optionの中にOptionが入った「二重の箱」
```

になる。`half`自身が既に`Some`/`None`を判断しているのに、`map`が外からもう1枚`Some`を
被せてしまうのが原因。

**`.and_then(half)`で呼ぶと:**

```rust
let b = x.and_then(half);
```

`and_then`は「中身があれば関数に渡し、**その関数の返り値をそのまま返す**（もう1枚
被せない）」動きをする。`half(4)`が`Some(2)`を返せば、`b`もそのまま

```
b = Some(2)   ← 1重で済む
```

になる。

**使い分けの基準**: 渡す関数（クロージャ）自身が`Option`を返すなら`and_then`、
ただの値を返すなら`map`。

### 元のコードに当てはめる

```rust
.and_then(|(handle, _)| images.get(handle))
```

ここでは `(handle, _)`（タプルを分解して `Handle` だけ使う）を受け取り、
`images.get(handle)`（＝ `Assets<Image>` から実際のピクセルデータを引く）を呼んでいる。
`images.get(...)`自体が`Option<&Image>`を返す関数（＝さっきの`half`と同じ立場）なので、
`map`ではなく`and_then`を使っている。結果は`Option<&Image>`（ブロック画像のピクセルデータ、
無ければ`None`）で、`Option<Option<&Image>>`のような二重にはならない。

## 4. `Option::zip` — 2つの `Option` を、両方揃ったときだけ1つに

```rust
.zip(images.get(&background_handle))
```

```
Some(a).zip(Some(b)) → Some((a, b))
どちらか一方でも None → None
```

「ブロック画像のピクセルデータ」と「背景画像のピクセルデータ」、**両方揃っているときだけ**
タプルにまとめる。

## 5. `Option::and_then`（2回目）— 揃った2つを使って本処理を呼ぶ

```rust
.and_then(|(brick_img, background_img)| {
    diff_brick_layout(background_img, brick_img, paddle_y, cell_size)
})
```

両方揃っていれば（＝ `Some((brick_img, background_img))` なら）、それを使って
`diff_brick_layout` を呼ぶ。`diff_brick_layout` 自体も `Option<BrickLayout>` を
返す関数（＝3節の`half`と同じ立場）なので、ここでも`map`ではなく`and_then`を使う。

## 6. `Option::flatten` — 二重になった `Option` を1段外す

`1節`の `.then(|| { ... })` は、中の処理（`Option<BrickLayout>` を返す）を
さらに `Option` で包むので、ここまで全体では `Option<Option<BrickLayout>>` になっている。
`.flatten()` はこの入れ子を1段外すだけ。

```
Some(Some(layout)) → Some(layout)
Some(None)          → None
None                → None
```

## 7. `?` 演算子で書き直すと（等価な別解）

読みやすさのための書き直しで実際のコードではないが、こう書いても全く同じ意味になる。

```rust
let diffed: Option<BrickLayout> = if background_was_overridden {
    (|| {
        let (handle, _) = brick_image.as_ref()?;
        let brick_img = images.get(handle)?;
        let background_img = images.get(&background_handle)?;
        diff_brick_layout(background_img, brick_img, paddle_y, cell_size)
    })()
} else {
    None
};
```

`?` は「`None` ならそこで即座に関数（ここではその場で定義したクロージャ）から抜けて
`None` を返す」という意味なので、`and_then` の連鎖と同じ「どれか1つでも欠けたら
諦める」という流れになっている。元のコードは `?` を使う代わりに `and_then` / `zip`
をメソッドチェーンでつないでいる、というだけの違い。`?` 演算子そのものの解説は
[[20260731_question-mark-operator-early-return]] を参照。

## 8. まとめ表

| メソッド | 意味 |
|---|---|
| `bool::then(\|\| x)` | `true` なら `Some(x)`（`x` は遅延評価）、`false` なら `None` |
| `Option::as_ref()` | `Option<T>` → `Option<&T>`（所有権を渡さず覗くだけ） |
| `Option::and_then(f)` | `Some(v)` なら `f(v)`（`f` 自体が `Option` を返す）、`None` ならそのまま `None` |
| `Option::zip(other)` | 両方 `Some` のときだけ `Some((a, b))`、どちらかが `None` なら `None` |
| `Option::flatten()` | `Option<Option<T>>` → `Option<T>` |

## 9. 余談: `half` を `if`/`else` を使わずに書く

3節の`half`はあえて`if`/`else`で書いたが（`map`/`and_then`の説明と混ざらないようにするため）、
`Option`のcombinatorだけで書き直すこともできる。**この節は3節の説明とは独立した別の話**であり、
以後の節では使わない。

```rust
fn half(n: i32) -> Option<i32> {
    Some(n).filter(|n| n % 2 == 0).map(|n| n / 2)
}
```

- `Some(n)`: まず必ず値ありの状態から始める
- `.filter(|n| n % 2 == 0)`: 偶数でなければ`None`に変える
- `.map(|n| n / 2)`: 残っていれば2で割る（ここでの`map`は`|n| n / 2`という**ただの`i32`を
  返すクロージャ**に対して使っているので、3節で説明した「二重になるパターン」には該当しない）

似た書き方として、`bool`から直接`Option`を作る`then`/`then_some`（1節）を使う版もある。

```rust
fn half(n: i32) -> Option<i32> {
    (n % 2 == 0).then_some(n / 2)
}
```

どちらも動作は同じで、単純な条件分岐なら元の`if`/`else`の方が読みやすいという意見も多い
（Rustの`if`/`else`は式なので、これ自体も十分「関数的」ではある）。`bool`から`Option`を
作りたい場面では`then`/`then_some`が定型的で意図が伝わりやすい。

## 関連

- 判定全体の文脈（何のための処理か） → [[20260802_brick-diff-auto-layout]]
- `?` 演算子による早期リターン → [[20260731_question-mark-operator-early-return]]
- 同じ `setup.rs` の `Option::take` / `unwrap_or_else` → [[20260729_option-take-unwrap-or-else]]
- `Option` の `match` 分解 → [[20260725_rust-match-and-pattern-binding]]
