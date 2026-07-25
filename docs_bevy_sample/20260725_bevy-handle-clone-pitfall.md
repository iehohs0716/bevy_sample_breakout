# Bevy：`Handle::clone` と参照カウント共有（`image.clone()` の落とし穴）

日付: 2026-07-25

`build_brick_material`（`rendering.rs:131`）で `BrickFill::Textured` の `image` を `ColorMaterial`
に渡す際、`image.clone()` ではなく **`Handle::clone(image)`** と書いている。これは単なる好みでは
なく、`image.clone()` だと **型が合わずコンパイルエラー（E0308）** になるのを避けるため。
`Handle<Image>` の正体（参照カウント共有）と、メソッド解決の落とし穴を整理する。

前提知識:
- `image` が `&Handle<Image>` として束縛される仕組み（`match` の分解束縛） → [[20260725_rust-match-and-pattern-binding]]
- `BrickFill` / `ColorMaterial` を使うメッシュ描画の全体像 → [[20260725_bevy-dynamic-mesh-build-brick-mesh]]
- `Assets<T>` / `Handle<T>` に登録して使う定石 → [[20260723_bevy-assets-handle-add-pattern]]
- 参照・デリファレンス周りの型の扱い → [[20260723_deref-newtype-vs-dot-zero]]

---

## 1. `Handle<Image>` とは：実体を指す軽い参照札

`Handle<Image>` は **画像の実体（ピクセルデータ）ではなく**、`Assets<Image>` に保管された実体を
指す **軽い参照札**。内部は **参照カウント（`Arc` 相当）** を持つ。

- `clone` してもカウントが +1 されるだけで、**同じ実体を共有**する。画像ピクセルは複製されない。
- したがって `Handle` の `clone` は **コストほぼゼロ**（札を 1 枚増やすだけ）。複数のブロックが
  同じ画像を貼るとき、実体は 1 つのまま各所が `Handle` を持ち合う。

「`clone` = 実体の重いコピー」という一般的な直感とは違い、`Handle::clone` は安価な参照共有である、
という点がまず前提。

## 2. 問題の核心：`image` は `&Handle<Image>`

`build_brick_material` は `fill: &BrickFill` を `match` する。参照越しの分解束縛なので、
`Textured { image, .. }` で束縛される `image` の型は **`&Handle<Image>`（参照）** になる
（詳細は [[20260725_rust-match-and-pattern-binding]]）。この「参照であること」が落とし穴の起点。

## 3. `image.clone()` の落とし穴

`image: &Handle<Image>` に対して `image.clone()` と書くと、**期待と違うものが返る**。

Rust のメソッド解決では、`&T` に対して常に生えている **ブランケット実装の `Clone`**
（`impl<T> Clone for &T` 相当＝「参照そのものをコピーして `&T` を返す」）が先に選ばれる。
その結果 `image.clone()` は `Handle<Image>` ではなく **`&Handle<Image>` を返してしまう**。

- `image`（`&Handle<Image>`）の `.clone()` → `&Handle<Image>`（参照のコピー）
- 本当に欲しいのは `Handle<Image>`（`Handle` 自身の複製 = 参照カウント +1）

自動デリファレンスより「参照自身に生える `Clone`」が優先されるため、`Handle` の `Clone` には
届かない。

## 4. なぜ E0308（型不一致）になるか

`ColorMaterial.texture` の型は `Option<Handle<Image>>` で、`Some(...)` の中に入れるべき値は
`Handle<Image>`。ところが `image.clone()` は `&Handle<Image>` を返すので、
`Some(image.clone())` は `Some(&Handle<Image>)` になり、型が合わない:

```
error[E0308]: mismatched types
  expected `Handle<Image>`, found `&Handle<Image>`
```

- **E0308** は「型が合わない（mismatched types）」を表す rustc のエラーコード。`expected`（要求される型）
  と `found`（実際に来た型）を並べて表示する。ここでは expected `Handle<Image>` / found
  `&Handle<Image>`。参照が 1 枚余分、というズレ。
- `rustc --explain E0308` でコードの一般的な説明を読める。

## 5. 解決：`Handle::clone(image)` で `Handle` の `Clone` を明示

```rust
// game_engine/src/rendering.rs:134
// `image` は `&Handle<Image>`。`image.clone()` は `&T` に常に生える `Clone`（参照自体の
// コピー）に解決されて `&Handle<Image>` を返してしまう（E0308）ため、`Handle::clone` を
// 明示して `Handle<Image>` 本体を複製する。
BrickFill::Textured { image, .. } => ColorMaterial {
    texture: Some(Handle::clone(image)),
    ..default()
},
```

`Handle::clone(image)` は **フルパス（関数呼び出し）記法** で `Clone` の実装を明示指定する
（`<Handle<Image> as Clone>::clone(image)` と同義）。これにより「参照に生えた `Clone`」ではなく
**`Handle` 自身の `Clone`** が選ばれる。

- `Clone::clone` のシグネチャは `fn clone(&self) -> Self`。`&self` を取るので、
  `image: &Handle<Image>` を **そのまま**渡せて、戻り値は `Handle<Image>`。
- 中身は 1 節の通り「札を 1 枚増やす（参照カウント +1）」だけで、画像コピーではない。

## 6. 比較

| 書き方 | 選ばれる `Clone` | 戻り値の型 | 結果 |
|---|---|---|---|
| `image.clone()` | `&T` に生えるブランケット `Clone`（参照のコピー） | `&Handle<Image>` | `Some(&Handle)` が `Option<Handle>` に入らず **E0308** |
| `Handle::clone(image)` | `Handle<Image>` 自身の `Clone` | `Handle<Image>` | `Some(Handle)` として OK（参照カウント +1） |

同じ「クローン」でも、メソッド構文 `.clone()` は参照側の実装に横取りされる。フルパス
`Handle::clone(...)` で対象を名指しすることで、意図した `Handle` の複製になる。

## 7. まとめ

- `Handle<Image>` は実体を指す軽い参照札。`clone` は参照カウント +1 の安価な共有で、画像は複製されない。
- `match` の参照越し束縛で `image` は `&Handle<Image>`。この参照に対する `image.clone()` は、
  `&T` に生えるブランケット `Clone` に解決されて `&Handle<Image>` を返す。
- 期待される `Handle<Image>` と `&Handle<Image>` が食い違い E0308（型不一致）になる。
- 解決は `Handle::clone(image)`（フルパス記法）。`Handle` の `Clone` を明示指定して `Handle<Image>` を
  得る。`&self` を取るので参照をそのまま渡せる。
