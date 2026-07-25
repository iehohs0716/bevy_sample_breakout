# Bevy：動的メッシュでブロックを描く（`build_brick_mesh`）

日付: 2026-07-25

ブロックは Sprite ではなく **動的に組み立てた `Mesh`（`Mesh2d`）+ `ColorMaterial`** で描画している。
`rendering.rs` の `build_brick_mesh` が、破れた辺のギザギザ輪郭を含むメッシュを毎回組み立てる。
Bevy の `Mesh` が「頂点属性」と「インデックス（つなぎ方）」でどう構成され、`build_brick_mesh` が
それをどう使っているかを、実コードの行番号付きで追う。

前提知識:
- 破れた辺の輪郭生成（中点変位法）とブロックのコンポーネント設計 → [[20260725_brick-torn-edge-midpoint-displacement]]
- 破壊イベント → 再描画への流れ → [[20260725_brick-destroyed-observer-event-lifecycle]]
- 当たり判定（ブロックは `Brick.size` 基準） → [[20260723_check-for-collisions-system]]
- 画像を切り出して貼る方式（UV の考え方の前提） → [[20260715_brick-image-rendering]]
- `Assets<T>` / `Handle<T>` に登録して使う定石（`meshes.add`） → [[20260723_bevy-assets-handle-add-pattern]]

---

## 0. 前提リファクタ：中点変位法の実行部を `tear.rs` へ分離

本ノートの直前に、ギザギザ生成（中点変位法）の実行部を `rendering.rs` から新モジュール
`src/tear.rs` へ切り出した。

- `tear.rs` は入口 `pub fn push_torn_edge(cell, edge_index, start, end, out)`（`tear.rs:67`）だけを
  公開し、内部で `seed_for` → `TearRng::new` → `midpoint_displace` と `TEAR_DEPTH` /
  `TEAR_ROUGHNESS` の使用を隠蔽する。`TearRng`（`tear.rs:13`）と `midpoint_displace`
  （`tear.rs:47`）は private、`seed_for`（`tear.rs:35`）はテストのため `pub`。
- `rendering.rs` 側の `build_brick_mesh` は破れ辺で `crate::tear::push_torn_edge(...)`
  （`rendering.rs:181`）を 1 行呼ぶだけになり、`TEAR_DEPTH` / `TEAR_ROUGHNESS` の import も削除した。
- `main.rs` に `mod tear;`（`main.rs:22`）を追加。`seed_for` の決定性テストは `tear.rs` へ移設。
- `cargo check`（wasm）・`cargo test`（4 件）パス済み。

これにより `rendering.rs` は「Mesh の組み立て」という主関心に集中し、ギザギザの数式は `tear.rs`
に閉じる。以降はこの分離を前提に `build_brick_mesh` を読む。

## 1. Bevy の `Mesh` の構成要素

Bevy の `Mesh` は大きく 2 つでできている:

1. **頂点属性（vertex attributes）**: 位置・法線・UV など、**種類ごとに別々の配列**で持つ。
   同じ添字が同じ頂点を指す「平行配列」。
2. **トポロジ + インデックス**: 頂点をどうつないで面（三角形）にするか。インデックスは
   頂点配列への添字の並びで、トポロジがその並びの解釈方法を決める。

つまり「点の集合（属性）」と「つなぎ方（トポロジ + インデックス）」を別々に与えて 1 枚のメッシュ
にする。

## 2. `Mesh::new`：トポロジと保持先

```rust
// game_engine/src/rendering.rs:209
Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
```

- **`PrimitiveTopology::TriangleList`**: インデックスを **3 個ずつ区切って**、各組を 1 枚の三角形と
  みなすモード。`[0, a, b, 0, c, d, ...]` なら `(0,a,b)`・`(0,c,d)`… が三角形。
- **`RenderAssetUsages::default()`**: メッシュデータを **GPU と CPU の両方に保持**する。
  CPU 側にも残るので、`redraw_broken_bricks` が `meshes.get_mut(...)` でメッシュ本体を後から
  書き換えられる（6 節）。GPU 専用にするとこの CPU 側書き換えができない。

## 3. 頂点属性：3 本の平行配列

`build_brick_mesh` は 3 種類の属性を、**同じ長さ・同じ添字が同じ頂点** を指す平行配列として作る。

```rust
// game_engine/src/rendering.rs:186
let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n + 1);
let mut normals: Vec<[f32; 3]> = Vec::with_capacity(n + 1);
let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(n + 1);

// 先頭(添字0)は必ず中心
positions.push([0.0, 0.0, 0.0]);
normals.push([0.0, 0.0, 1.0]);
uvs.push(vertex_uv(Vec2::ZERO, size, fill));

// 以降が輪郭点（四隅＋破れた辺のギザギザ点）
for p in &boundary {
    positions.push([p.x, p.y, 0.0]);
    normals.push([0.0, 0.0, 1.0]);
    uvs.push(vertex_uv(*p, size, fill));
}
```

| 属性 | 型 | 中身 |
|---|---|---|
| `Mesh::ATTRIBUTE_POSITION` | `[f32; 3]` | ローカル座標（z=0、中心原点）。ワールド位置は `Transform` 側が持つ |
| `Mesh::ATTRIBUTE_NORMAL` | `[f32; 3]` | 2D なので全頂点 `[0, 0, 1]` 固定（画面手前向き） |
| `Mesh::ATTRIBUTE_UV_0` | `[f32; 2]` | `vertex_uv` がローカル座標→画像切り出し矩形へマッピング。`Color` 塗りのときは `[0, 0]` |

- **先頭（添字 0）は必ず中心 `Vec2::ZERO`**。以降が輪郭点（四隅＋破れた辺のギザギザ点）で、
  ギザギザ点は `tear::push_torn_edge` が `boundary` に積む（[[20260725_brick-torn-edge-midpoint-displacement]]）。
- `Mesh::ATTRIBUTE_POSITION` / `NORMAL` / `UV_0` は **Bevy 標準の属性 ID**。標準マテリアル
  （`ColorMaterial`）はこれらの標準属性をそのまま読んで描画できるので、専用シェーダは要らない。
- `vertex_uv`（`rendering.rs:148`）はローカル座標を UV に変換する。`Textured` なら
  ブロックが覆う画像領域の切り出し矩形へ、`Color` なら `[0, 0]`（使われない）。

## 4. インデックス：扇形三角形分割（fan triangulation）

```rust
// game_engine/src/rendering.rs:200
let mut indices: Vec<u32> = Vec::with_capacity(n * 3);
for i in 0..n {
    let a = (i + 1) as u32;
    let b = (((i + 1) % n) + 1) as u32;
    indices.push(0); // 常に中心
    indices.push(a); // 輪郭点 i
    indices.push(b); // 次の輪郭点（最後は最初へ折り返す）
}
```

- 添字 0（中心）を固定の要とし、輪郭点は添字 `1..=n`。三角形「中心 → 輪郭点 i → 次の輪郭点」を
  輪郭 1 周ぶん張る。これが **扇形三角形分割（fan triangulation）**。
- `b` の `(i + 1) % n` で最後の輪郭点と最初の輪郭点をつなぎ、輪郭を閉じる。
- 成立条件は **star-shaped**（中心から全輪郭点が直接見える）。破れが暴れて輪郭が凹むと扇分割が
  自己交差して破綻するため、`TEAR_ROUGHNESS` で振れ幅を抑え、`build_brick_mesh_is_star_shaped_*`
  テスト（`rendering.rs:230` / `rendering.rs:243`）で担保している。

## 5. ビルダーパターンで組み立て

```rust
// game_engine/src/rendering.rs:209
Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
```

- `with_inserted_attribute` / `with_inserted_indices` は **`Mesh` 自身を返す** ので、
  メソッドチェーンで属性とインデックスを差し込みながら 1 つの式で組み立てられる。
- `Indices::U32(indices)` は、インデックスを **u32** で持つ指定（頂点数が多くても足りる幅）。

## 6. 全体の流れと再描画

組み立てから spawn、そして再描画までの流れ:

```
tear::push_torn_edge         破れた辺のギザギザ輪郭点を boundary に積む
        │
平行配列を作る               positions/normals/uvs（先頭=中心、以降=輪郭点）
        │
fan で indices を作る         [0, i, i+1] を 1 周ぶん（U32）
        │
Mesh::new + 3属性 + indices   build_brick_mesh が Mesh を返す
        │
meshes.add(mesh) → Mesh2d     spawn_brick が Handle にして spawn（rendering.rs:104-108）
```

再描画（隣が壊れて `BrokenEdges` が変わったとき）は、`build_brick_mesh` を **もう一度呼んで
メッシュ本体を丸ごと差し替える**:

```rust
// game_engine/src/rendering.rs:120
pub fn redraw_broken_bricks(
    mut meshes: ResMut<Assets<Mesh>>,
    bricks: Query<(&Brick, &BrokenEdges, &Mesh2d), Changed<BrokenEdges>>,
) {
    for (brick, broken, mesh2d) in &bricks {
        if let Some(mut mesh) = meshes.get_mut(&mesh2d.0) {
            *mesh = build_brick_mesh(brick.size, brick.cell, broken, &brick.fill);
        }
    }
}
```

`meshes.get_mut` で既存の `Mesh` 本体を取り出して `*mesh = ...` で置き換えられるのは、
2 節の `RenderAssetUsages::default()` により **CPU 側にもデータが残っている** から。
`Changed<BrokenEdges>` で対象を絞るので、変化したブロックだけ作り直す
（発火から再描画までの順序保証は [[20260725_brick-destroyed-observer-event-lifecycle]]）。

## 7. なぜ Sprite ではなく動的メッシュか

```rust
// game_engine/src/rendering.rs:7
//! ブロックは Sprite ではなく Mesh2d(動的メッシュ) + ColorMaterial で描画する。壁・パドルと違い
//! ブロックは破壊された隣との接触面だけを中点変位法のギザギザ輪郭に再構築する必要があり、
//! それには頂点を自前で持てるメッシュが要る。
```

壁・パドルは矩形で足りるので `Sprite` のまま。しかしブロックは **破壊面だけをギザギザに作り替える**
必要があり、Sprite（矩形しか描けない）では表現できない。頂点を自前で持てる動的メッシュだからこそ、
辺の一部だけを凹凸のある輪郭に置き換えられる。役割が変わらないもの（壁・パドル・背景）は Sprite の
ままにし、変える必要があるブロックだけメッシュ化している。

## 8. まとめ

- Bevy の `Mesh` は「頂点属性（種類ごとの平行配列）」＋「トポロジ + インデックス」で構成される。
- `build_brick_mesh` は `TriangleList` + `RenderAssetUsages::default()`（CPU 保持）で作り、
  POSITION/NORMAL/UV_0 の 3 平行配列（先頭=中心、以降=輪郭点）と、扇形三角形分割の U32 インデックスを
  ビルダーチェーンで差し込む。
- 扇分割は star-shaped 前提。`TEAR_ROUGHNESS` で振れ幅を抑え、回帰テストで担保。
- CPU 保持のおかげで `redraw_broken_bricks` が `meshes.get_mut` + `*mesh = build_brick_mesh(...)` で
  丸ごと差し替えできる。ギザギザ生成の実行部は `tear.rs` に分離済み。
