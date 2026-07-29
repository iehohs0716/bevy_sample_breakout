# `Option::take` と `unwrap_or_else` の定番コンボ

日付: 2026-07-29

`setup.rs` のブロック配置確定でこう書いた1行の意味を整理する。Rust では特殊なテクニックではなく、
「共有リソースから所有権付きで値を取り出し、無ければデフォルトを作る」ときの教科書的な組み合わせ。

```rust
let brick_layout = brick_layout_override
    .0
    .take()                                             // ①
    .unwrap_or_else(|| default_brick_layout(paddle_y)); // ②
```

`brick_layout_override.0` の型は `Option<BrickLayout>`（値が入っているかもしれない箱）。

---

## ① `.take()` — 中身を抜き取り、跡地に `None` を残す

`Option::take` は、**箱の中身を所有権ごと自分がもらい、箱には `None` を置いていく**メソッド。
シグネチャは概念的に `fn take(&mut self) -> Option<T>`。

```rust
let mut slot = Some(5);
let got = slot.take(); // got == Some(5)、slot == None
```

### なぜ抜く必要があるのか

`brick_layout_override` は `ResMut<BrickLayoutOverride>`、つまり **借りているだけ**の共有リソース。
丸ごと move して自分のものにはできない。だが中身の `BrickLayout` は所有権ごと欲しい
（後で `GameAssets` に格納したりループで消費するため）。

`&mut` 越しに `Option` の中身だけを持ち出す正攻法が `take()`。所有権ルールとセットで頻出する。

- `into_inner()` などで丸ごと奪うのは、借用相手には使えない。
- 参照で借りる（`.as_ref()`）だけでは所有権が渡らず、後段で move できない。

---

## ② `.unwrap_or_else(...)` — 入っていればそれ、無ければ作る

`.take()` の結果はまだ `Option`。それを実際の値に落とすのがこれ。

- `Some(値)` → その値をそのまま使う
- `None`   → `|| default_brick_layout(paddle_y)` を呼んでデフォルトを生成する

### `unwrap_or` との使い分け

| メソッド | フォールバック値の評価 |
|---|---|
| `unwrap_or(x)` | `x` を**先に必ず**作ってから、要るか判断 |
| `unwrap_or_else(\|\| x)` | `None` のとき**だけ** `x` を作る（クロージャで遅延評価） |

ここでは `default_brick_layout(...)` を `Some` のときにまで毎回呼ぶのは無駄なので、
`unwrap_or_else` を選ぶ。副作用やコストのある生成処理なら基本こちら。

関連仲間: `unwrap_or_default()`（`Default` 実装があるとき）、`ok_or_else`（`Option`→`Result`）。

---

## まとめ

- `take` = 「借り物の `Option` から中身を所有権付きで抜く。跡地は `None`」
- `unwrap_or_else` = 「あればそれ、無ければ（そのときだけ）デフォルトを作る」
- 2つ合わせて「React(JS) が配置を渡していればそれ、無ければデフォルト配置」を1行で表す定番。

関連:
- 所有権・借用の背景 → [[20260728_rust-lifetimes-basics]]
- `match` での `Option` 分解 → [[20260725_rust-match-and-pattern-binding]]
