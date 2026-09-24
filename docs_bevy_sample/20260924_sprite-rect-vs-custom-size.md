# `Sprite.rect` と `Sprite.custom_size` の役割分担（両方指定した場合の実際の適用順序）

日付: 2026-09-24

`Sprite.custom_size` を画像本来のサイズより小さくすると縮小されるのかクロップされるのか、
また `rect` と `custom_size` を両方指定した場合どちらが優先されるのか、を Bevy 0.19.0 の
実ソース（`~/.cargo/registry/src/.../bevy_sprite_render-0.19.0/src/render/mod.rs`）を直接
読んで確認した記録。背景 Sprite の配置は [[20260803_image-top-alignment]]、ブロックの
テクスチャ切り出しは [[20260715_brick-image-rendering]] を参照（**ブロックは `Sprite` では
なく `Mesh2d` + 自前 UV で同等の切り出しを実現している**点は 3 節を参照）。

---

## 1. 結論

`rect` と `custom_size` は「競合して片方が勝つ」関係ではなく、**役割が完全に分かれていて
両方適用される**。

- `custom_size` だけを指定した場合: 画像**全体**を、指定サイズにぴったり収まるよう
  引き伸ばして描く（クロップではなく常にリサイズ）。アスペクト比が違えば縦横別々に
  伸縮するので**歪む**。
- `rect` は「テクスチャのどのピクセル範囲をサンプリングするか」（＝クロップ）を決める。
- `custom_size` は「最終的に画面上で何ワールド単位の大きさで描くか」を決める。
- 両方指定した場合の見た目は「`rect` で切り出した範囲の絵を、`custom_size` の矩形に
  ぴったり収まるよう引き伸ばして描く」。

## 2. 実ソースでの確認（`render/mod.rs:735-767`）

```rust
// By default, the size of the quad is the size of the texture
let mut quad_size = batch_image_size;
let mut texture_size = batch_image_size;

// rect が Some なら、UV（サンプリング範囲）と quad_size をいったん rect のピクセルサイズに更新
let mut uv_offset_scale = if let Some(rect) = rect {
    let rect_size = rect.size();
    quad_size = rect_size;
    texture_size = rect_size;
    Vec4::new(/* rect範囲に対応するUVオフセット・スケール */)
} else {
    Vec4::new(0.0, 1.0, 1.0, -1.0) // 画像全体のUV
};

// ...flip_x/flip_y の処理...

// Override the size if a custom one is specified
quad_size = custom_size.unwrap_or(quad_size);
```

適用順序:

1. `rect` は常に「どのピクセル範囲をサンプリングするか」（`uv_offset_scale`）を決める。
   これは `custom_size` の有無に関係なく必ず適用される。同時に `quad_size`
   （描画サイズ）の**仮の値**として `rect.size()`（ピクセル単位）が入る。
2. 最後にコード上で明示的に `quad_size = custom_size.unwrap_or(quad_size)` が実行される。
   `custom_size` が `Some` なら、1. で決めた仮の `quad_size`（`rect.size()`）を
   問答無用で上書きする。

つまり「どちらが先に判定されるか」というより、**`rect` は UV（切り出し範囲）の決定にしか
関与せず、描画サイズの決定権は常に `custom_size` 側にある**（`custom_size` 未指定なら
`rect` のピクセルサイズ、`rect` も無ければ元画像のピクセルサイズにフォールバック）。

## 3. 本リポジトリでの `Sprite.rect` の使用状況: ブロックは `Sprite` 自体を使わない

**ブロックは `Sprite` を一切使っていない**（`common/brick.rs` で `Mesh2d` +
`MeshMaterial2d` を spawn している。理由は破壊された辺だけをギザギザの輪郭に変形させる
必要があり、矩形固定の `Sprite` では表現できないため）。

切り出しに相当する処理は次のように**自前の UV 座標**で実現している（詳細は
[[20260715_brick-image-rendering]]）:

1. `common/brick/texture_crop.rs` の `brick_uv_rect` が、ブロックが覆う領域に対応する
   画像内の範囲を 0..1 の UV 矩形として計算する（内部は `util::inscribed_source_rect`）。
2. `common/brick/mesh.rs` の `vertex_uv` が、メッシュの各頂点のローカル座標をその
   `uv_rect` 範囲内に線形マッピングして UV 値を作る。
3. `build_brick_mesh` がこの UV 値を `Mesh::ATTRIBUTE_UV_0` としてメッシュに埋め込む。

数学的な意味（画像のどの範囲を切り出すか）は `Sprite.rect` と同じだが、`Sprite` の機能
ではなく `Mesh2d` レベルの自前実装である点が異なる。

**実際に `Sprite.rect` を使っている箇所は本リポジトリには無い。** 背景 Sprite
（`systems/setup.rs`）も `rect` を指定せず、常に画像全体を使って `custom_size` で
縮小表示しているだけ（[[20260803_image-top-alignment]] 参照）。

## 4. 関連ファイル

- `game_engine/src/systems/setup.rs` — 背景 Sprite（`rect` 未使用、`custom_size` のみ）
- `game_engine/src/common/brick/texture_crop.rs` — `brick_uv_rect`（切り出し範囲の計算）
- `game_engine/src/common/brick/mesh.rs` — `vertex_uv` / `build_brick_mesh`（UV をメッシュに埋め込む）
- `~/.cargo/registry/src/index.crates.io-.../bevy_sprite_render-0.19.0/src/render/mod.rs:735-767`
  — `rect`/`custom_size` の実際の適用順序（一次情報）

## 5. 関連ドキュメント

- [[20260715_brick-image-rendering]] — ブロックの画像切り出し設計（`Mesh2d` + UV 方式）
- [[20260803_image-top-alignment]] — 背景 Sprite の配置（`contain_fit` は常にクロップしない）
- [[20260723_bevy-assets-handle-add-pattern]] — `Assets<T>::add` と `Handle<T>` の関係
