# Rust の `match` とパターン束縛（`build_brick_material` を例に）

日付: 2026-07-25

`rendering.rs` の `build_brick_material`（ブロックの塗り方 `BrickFill` から `ColorMaterial` を作る
関数）を題材に、Rust の `match` と **パターンによる分解束縛（destructuring）** を整理する。
`enum` の網羅性チェック、タプル型／構造体型バリアントの束縛、そして紛らわしい 2 種類の `..` を
区別する。

前提知識:
- 束縛された `image` を `Handle::clone` する理由（本ノートの続き） → [[20260725_bevy-handle-clone-pitfall]]
- `BrickFill` を使う動的メッシュ描画の全体像 → [[20260725_bevy-dynamic-mesh-build-brick-mesh]]
- 画像を切り出して貼る方式（`uv_rect` の意味） → [[20260715_brick-image-rendering]]
- 参照・デリファレンス周りの型の話（`*` の扱い） → [[20260723_deref-newtype-vs-dot-zero]]

---

## 1. 題材のコードと enum 定義

```rust
// game_engine/src/rendering.rs:131
fn build_brick_material(fill: &BrickFill) -> ColorMaterial {
    match fill {
        BrickFill::Color(color) => ColorMaterial::from(*color),
        BrickFill::Textured { image, .. } => ColorMaterial {
            texture: Some(Handle::clone(image)),
            ..default()
        },
    }
}
```

```rust
// game_engine/src/components.rs:81
#[derive(Clone)]
pub enum BrickFill {
    /// `image` のうち `uv_rect`(画像全体を 0..1 に正規化した UV 矩形)を貼る。
    Textured { image: Handle<Image>, uv_rect: Rect },
    Color(Color),
}
```

`BrickFill` は 2 バリアントの enum。**`Color` はタプル型バリアント**（`Color` を 1 個持つ）、
**`Textured` は構造体型バリアント**（名前付きフィールド `image` / `uv_rect` を持つ）。

## 2. `match` の基本と網羅性チェック

- `match` は **上から順にパターン照合** し、最初に一致した腕（arm）の右辺（`=>` の後）を実行する。
- Rust の `match` は **網羅的でなければコンパイルエラー**。`enum` の全バリアントを（または `_`
  ワイルドカードで）カバーしないと `error[E0004]: non-exhaustive patterns` になる。
  `build_brick_material` は `Color` と `Textured` の両方を書いているので網羅されている。
- 網羅性チェックのおかげで、後から enum にバリアントを足すと「その `match` を直し忘れる」ことを
  コンパイラが検出してくれる（安全側に倒れる）。

## 3. 分解束縛（destructuring）

パターンは「一致するか」を判定するだけでなく、**一致と同時に中身を取り出して名前を付けられる**。

### 3.1 タプル型バリアント：`BrickFill::Color(color)`

```rust
BrickFill::Color(color) => ColorMaterial::from(*color),
```

`Color(color)` は「このバリアントの中身を `color` という名前で束縛する」。ここで `fill` の型は
**`&BrickFill`（参照）** なので、束縛される `color` も **`&Color`（参照）** になる（5 節）。
`Color` は `Copy` なので、`*color` でデリファレンスして実体（`Color`）を取り出し
`ColorMaterial::from(...)` に渡している。

### 3.2 構造体型バリアント：`BrickFill::Textured { image, .. }`

```rust
BrickFill::Textured { image, .. } => /* ... image を使う ... */
```

構造体型バリアントはフィールド名で分解する。`{ image }` は `{ image: image }` の **省略記法**
（フィールド名と同じ変数名に束縛するときはコロン以降を省ける）。`fill` が参照なので
`image` の型は **`&Handle<Image>`**（この参照越しの束縛が次のレポートの落とし穴の起点になる →
[[20260725_bevy-handle-clone-pitfall]]）。`uv_rect` はこの関数では使わないので、後述の `..` で
無視している。

## 4. 2 種類の `..` を区別する（重要）

同じ `..` という記号だが、**文脈で意味がまったく違う**。混同しやすいので明確に分ける。

| 記法 | 文脈 | 意味 |
|---|---|---|
| `Textured { image, .. }` | **パターン内**の `..` | 「残りのフィールドは無視する」。`uv_rect` を書かなくてよくなる |
| `ColorMaterial { texture: ..., ..default() }` | **構造体生成**の `..default()` | 「残りのフィールドは `Default` 値で埋める」struct update syntax |

- **パターン内の `..`**: `BrickFill::Textured { image, .. }` の `..` は「`image` 以外のフィールド
  （ここでは `uv_rect`）は照合対象にしない＝無視する」の意。これを書かないと
  「フィールド `uv_rect` が指定されていない」とコンパイラに怒られる（構造体パターンは全フィールドを
  列挙するのが原則で、`..` がその免除）。
- **構造体生成の `..default()`**: `ColorMaterial { texture: Some(...), ..default() }` の `..default()`
  は「明示していない残りのフィールドを `Default::default()` の値で埋める」struct update syntax。
  これは **値を作る側** の記法で、パターン（分解する側）の `..` とは別物。

同じ 2 ドットでも、片方は「取り出さない」、もう片方は「埋める」。役割が逆向きである点に注意。

## 5. 参照越しの `match`

`match fill`（`fill: &BrickFill`）のように **参照を `match`** すると、パターンで束縛される変数も
基本的に **参照** になる（Rust の match ergonomics）。

- `BrickFill::Color(color)` → `color: &Color`
- `BrickFill::Textured { image, .. }` → `image: &Handle<Image>`

だから `Color` 側は `*color` で実体を取り出す必要があり（`Color` は `Copy` なので `*` で値が得られる）、
`Textured` 側の `image` も `&Handle<Image>` として扱う。この「`image` が `&Handle<Image>` である」
ことが、`image.clone()` が期待通りに動かない原因につながる。その詳細（`Handle::clone` を明示する
理由、`.clone()` の落とし穴、E0308）は別レポートに分けた → [[20260725_bevy-handle-clone-pitfall]]。

## 6. まとめ

- `match` は上から照合し最初に一致した腕を実行。enum は全バリアント網羅が必須（コンパイラが担保）。
- パターンは一致と同時に中身を束縛できる。タプル型は `Color(color)`、構造体型は
  `Textured { image, .. }`（`{ image }` は `{ image: image }` の省略）。
- `..` は文脈で別物: パターン内 `..` =「残りを無視」、構造体生成 `..default()` =「残りを Default で埋める」。
- 参照を `match` すると束縛変数も参照（`&Color` / `&Handle<Image>`）。この参照越しの束縛が
  `Handle::clone` を明示する必要につながる（→ [[20260725_bevy-handle-clone-pitfall]]）。
