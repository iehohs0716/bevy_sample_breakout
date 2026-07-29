# Rust の `From` / `Into` トレイト（型変換イディオム）

日付: 2026-07-25

`WallLocation`（壁を作れる 3 辺の制約型）から `ArenaEdge`（アリーナ 4 辺の幾何型）への変換に
`impl From<WallLocation> for ArenaEdge` を実装し、`Wall::new` の中で
`let edge: ArenaEdge = location.into();` と書いている。これを題材に、Rust の標準的な型変換トレイト
`From` / `Into` のイディオムを整理する。

前提知識:
- `.into()` と紛らわしい `into_inner()`（別物: ラッパーの中身取り出し） → [[20260723_bevy-single-into-inner]]
- 参照越しの `match` と分解束縛（`from` 実装内の match の読み方） → [[20260725_rust-match-and-pattern-binding]]
- メソッド解決が絡む型の落とし穴（`Handle::clone` の例） → [[20260725_bevy-handle-clone-pitfall]]
- 型・参照まわりの扱い（`.0` / `*`） → [[20260723_deref-newtype-vs-dot-zero]]

---

## 0. 設計文脈：`ArenaEdge`（幾何）と `WallLocation`（制約）の分離

このセッションで、アリーナ境界の幾何を `ArenaEdge` に一元化するリファクタを行った。役割分離
（`Wall` = 反射壁 / `DeathZone` = ライフ減）は維持したまま、幾何の定義だけを 1 箇所に集約している。

```rust
// game_engine/src/components.rs:113
pub enum ArenaEdge {
    Left,
    Right,
    Top,
    Bottom,
}
```

`ArenaEdge` は 4 辺の **幾何（position / size）だけ** を持つ純粋な型で、役割は持たない
（`components.rs:107-118`。`position()` は `:122`、`size()` は `:133`）。

```rust
// game_engine/src/components.rs:155
pub enum WallLocation {
    Left,
    Right,
    Top,
}
```

一方 `WallLocation` は **`Bottom` をあえて持たない**（`components.rs:151-159`）。これは
「壁は 3 辺にしか作れない＝下端の反射壁は表現不能」という制約を **型で表す番人**。列挙子が 3 つ
なのは重複ではなく制約そのもの。下端はライフを減らす `DeathZone` が担い、その幾何は
`ArenaEdge::Bottom` を直接使う（`components.rs:211-212`）。

この「制約（`WallLocation`）→ 幾何（`ArenaEdge`）」の橋渡しが `From` 実装。本レポートの主題は
この `From` / `Into` の使い方にある。

## 1. `From` / `Into` とは

どちらも「ある型 A から別の型 B を作る」型変換の標準トレイト。

```rust
// std（概念）
trait From<T> {
    fn from(value: T) -> Self;   // T から Self を作る
}
trait Into<T> {
    fn into(self) -> T;          // self を T に変換する
}
```

- `From<T>` は「`T` を受け取って自分自身（`Self`）を作る」。作られる側に実装する。
- `Into<T>` は「自分を `T` に変換する」。変換される側の視点。

今回の実装:

```rust
// game_engine/src/components.rs:163
impl From<WallLocation> for ArenaEdge {
    fn from(location: WallLocation) -> Self {
        match location {
            WallLocation::Left => ArenaEdge::Left,
            WallLocation::Right => ArenaEdge::Right,
            WallLocation::Top => ArenaEdge::Top,
        }
    }
}
```

`WallLocation` は 3 バリアントしか無いので、この `match` は 3 腕で網羅され、変換結果に
`ArenaEdge::Bottom` は **決して現れない**（`components.rs:161-162` のコメント）。

## 2. 核心：`From` を実装すると `Into` が自動でついてくる

`From` を実装するだけで、対応する `Into` が **自動で使える**ようになる。std に次のブランケット
実装があるため:

```rust
// std（ブランケット実装）
impl<T, U> Into<U> for T
where
    U: From<T>,
{
    fn into(self) -> U {
        U::from(self)
    }
}
```

「`U: From<T>` が成り立つなら、`T` は `Into<U>` を持つ」。つまり実装するのは `From` 側だけでよく、
`Into` は書かない（Rust の慣習）。

今回も `impl From<WallLocation> for ArenaEdge` を **1 つ書いただけ** で、`WallLocation` 側に何も
足さずに `location.into()` が使える:

```rust
// game_engine/src/components.rs:177
pub fn new(location: WallLocation) -> (Wall, Sprite, Transform) {
    let edge: ArenaEdge = location.into();
    // ... edge.position() / edge.size() で Transform を作る ...
}
```

`location.into()` の中身は実質 `ArenaEdge::from(location)`（ブランケット実装が `U::from(self)` を
呼ぶ）。

## 3. `.into()` の変換先は型注釈から推論される

`.into()` は「どの型に変換するか」を **文脈（型注釈）から推論**する。

```rust
let edge: ArenaEdge = location.into();
```

左辺の `: ArenaEdge` 注釈を見て、コンパイラは `Into<ArenaEdge>` を選ぶ。もし注釈が無いと、
`WallLocation` は理屈上いくつもの型へ `Into` し得る（将来別の `From` が増える可能性も含め）ため
**変換先を決められずコンパイルエラー**になる。

注釈を付けたくない／変換先を明示したいときは `from` を直接呼べばよい:

```rust
let edge = ArenaEdge::from(location);   // 型注釈不要
```

両者は等価（`into()` はブランケット実装経由で `from` を呼ぶだけ）。

| 書き方 | 特徴 | 向いている場面 |
|---|---|---|
| `ArenaEdge::from(location)` | 変換先を関数名で明示、型注釈が不要 | 変換先をはっきり見せたい／注釈を書きたくない |
| `location.into()` | 左から右へ読み下せる。変換先は型注釈頼み | 文脈で変換先が自明・代入先の型が既にある |

`Wall::new` では `let edge: ArenaEdge = ...` と代入先の型が明示されているので、読み下しやすい
`.into()` を選んでいる。

## 4. なぜ `Into` ではなく `From` を実装するか

1. **`From` を書けば `Into` が無料でついてくる**（2 節）。逆（`Into` を書いても `From` は付かない）は
   成り立たない。だから実装は `From` 側に寄せるのが定石。
2. **orphan rule（孤児ルール）**: トレイト実装は「トレイトか実装対象の型のどちらかが自クレートの
   もの」でないと書けない。`ArenaEdge` は自クレートの型なので `impl From<WallLocation> for ArenaEdge`
   が書ける（`From` は std のトレイトだが、`ArenaEdge` が自前の型なので OK）。仮に `Into` を外部型に
   対して実装しようとすると、この規則に引っかかりやすい。

## 5. Rust 特有のイディオムか

「型変換」という概念自体はどの言語にもある。しかし Rust の特徴は、

- **`From` 一本を書けば `Into` が自動生成され**、
- **`.into()` が型推論で変換先を決めて動く**、

という「トレイト + ブランケット実装」の組み合わせにある。Rust は暗黙の型変換を持たない代わりに、
この明示的で型安全な変換イディオムを多用する。std にも例が多い:

```rust
let s: String = "hello".into();   // &str -> String（String: From<&str>）
let s = String::from("hi");       // from 直呼び
let n: i64 = 42i32.into();        // i32 -> i64（i64: From<i32>）
```

## 6. この設計での効用

- `WallLocation`（制約）→ `ArenaEdge`（幾何）の橋渡しが、標準の `.into()` **1 行**で書ける
  （`Wall::new`, `components.rs:179`）。幾何の定義は `ArenaEdge` 1 箇所に集約されたまま。
- `From` の `match` は `WallLocation` の 3 バリアントしか扱わないので、**この変換から
  `ArenaEdge::Bottom` は絶対に生まれない**。「下端の反射壁」という誤用を型レベルで封じている。
- 役割分離（反射は `Wall`、ライフ減は `DeathZone`）は維持しつつ、幾何だけを共有できる。
  `DeathZone` は `ArenaEdge::Bottom` を直接使い、`Wall` は `From` 経由で `Bottom` を避ける。

## 7. まとめ

- `From<T>` は「`T` から `Self` を作る」、`Into<T>` は「`self` を `T` にする」標準変換トレイト。
- std のブランケット実装により、**`From` を実装すれば `Into` は自動**。実装は `From` 側だけでよい。
- `.into()` の変換先は型注釈から推論。注釈が無いと決められずエラー。`Type::from(x)` なら注釈不要で等価。
- `From` を選ぶ理由は「`Into` が無料で付く」＋「orphan rule 上、自クレート型 `ArenaEdge` になら実装可」。
- 本設計では制約型 → 幾何型の変換を `.into()` 1 行で書け、`Bottom` を生まない型安全を得ている。
