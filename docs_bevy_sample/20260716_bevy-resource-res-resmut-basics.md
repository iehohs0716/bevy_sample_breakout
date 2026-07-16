# Bevy 入門：Resource と `Res` / `ResMut` とは（読み方・使い分け・なぜ分ける）

日付: 2026-07-16

「`ResMut` や `Res` が正直何を言ってるのか全然分からない」という素朴な疑問への回答を整理した記録。
Bevy 初学者向けに、ECS のデータ置き場 → Resource → `Res`/`ResMut` の順で最小限だけ説明する。

system の引数がなぜ勝手に埋まるのか（依存性注入）は
[[20260716_background-rendering-and-bevy-system-params]] を参照。

---

## 1. 読み方（略語の正体）

- `Res<T>` = **Resource（リソース）** の略。「レズ」ではなく Resource。
- `ResMut<T>` = **Resource + Mutable（ミュータブル＝書き換え可能）**。「書き換えできるリソース」。

`Res` も `ResMut` も **同じ Resource へのアクセス券**で、違いは「読むだけ」か「書き換えていい」かだけ。

## 2. 前提：ECS のデータ置き場は 2 種類

Bevy は **ECS（Entity / Component / System）** で動く。データの置き場所が 2 つある。

### ① Component — 「モノ 1 個ずつ」に付くデータ

個々のオブジェクト（Entity）にくっつく。

```
ボール      → Transform(位置), Velocity(速度), Sprite(見た目)
パドル      → Transform, Sprite
ブロック#1  → Transform, Sprite, Collider
ブロック#2  → Transform, Sprite, Collider   （たくさんある）
```

### ② Resource — 「世界に 1 個だけ」の共有データ

どのオブジェクトにも属さない、ゲーム全体で 1 つだけあればいいデータ。

```
スコア         → Score(0)        ← 1 個だけ
画像の倉庫     → Assets<Image>   ← 1 個だけ
ファイル読込係 → AssetServer     ← 1 個だけ
```

スコアが分かりやすい例。点数はボールごと・ブロックごとにあるのではなく、ゲームに 1 つ。
こういう「1 個だけの共有データ」が Resource。

## 3. Resource は最初に「世界に登録」しておく

`main.rs` で登録する:

```rust
// game_engine/src/main.rs
.insert_resource(Score(0))                                        // スコアを 0 で登録
.insert_resource(BackgroundOverride(injected_background_image())) // 背景画像を登録
```

`insert_resource` = 「この Resource を世界に置いておいて」という宣言。一度置けば、
どの system からでも `Res` / `ResMut` で取り出せる。

## 4. `Res` と `ResMut` の使い分け

system の引数に型を書くと、Bevy が登録済み Resource を渡してくれる。

```rust
asset_server: Res<AssetServer>,      // 読むだけ           → Res
mut images: ResMut<Assets<Image>>,   // 書き換える(add する) → ResMut + mut
```

| | 意味 | いつ使う |
|---|---|---|
| `Res<T>` | Resource を **読み取り専用**で借りる | 値を見るだけ（例: `asset_server.load(...)`） |
| `ResMut<T>` | Resource を **書き換え可能**で借りる | 中身を変える（例: `images.add(...)` / `score.0 += 1`） |

`mut` は Rust の「後で書き換える変数」の印。中身を変える `ResMut` には `mut` が付き、
読むだけの `Res` には付かない。

## 5. なぜわざわざ 2 つに分けるのか（一番大事な理由）

**Bevy が複数の system を並列実行して高速化するため**。事故を防ぐルールがある:

- 読むだけ（`Res`）なら、何人が同時に読んでも安全 → **並列 OK**
- 書き換える人（`ResMut`）がいる間は、他の誰も触れない → **排他**

例：「スコアを書き換える system」と「スコアを表示する system」が同じ瞬間に動くと、
表示中に数字が書き換わって壊れうる。Bevy は引数の `Res` / `ResMut` を見て
「この 2 つは同じ `Score` を触るから同時に走らせず順番にやろう」と**自動判断**する。

つまり `Res` / `ResMut` は飾りではなく **「私はこの Resource を読むだけ／書き換えます」という
Bevy への申告**。この申告があるから安全に並列化できる。

## 6. まとめ

- **Resource** = 世界に 1 個だけの共有データ（スコア・倉庫・ファイル読込係など）
- **`Res<T>`** = 読むだけで借りる
- **`ResMut<T>`** = 書き換えて借りる（変数に `mut` も付く）
- 区別する理由 = Bevy が system を**安全に並列実行**するため

（対になる概念：個々のオブジェクトに付くデータは Component。1 個だけの共有データが Resource。）
