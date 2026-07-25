# Bevy 入門：Mesh・Material・UV座標とは何か（`Mesh2d` + `MeshMaterial2d<ColorMaterial>` の基本）

日付: 2026-07-25

「`Mesh2d` と `MeshMaterial2d<ColorMaterial>` を両方 spawn しないと画像が出ないのはなぜか」
「UV座標って何を表す数字なのか」という、Bevy/グラフィックスの前提知識が無い状態からの
素朴な疑問への回答を整理した記録。ブロックの動的メッシュ実装（`build_brick_mesh` など）を
読み解く前提になる基礎知識なので、先にこちらを読むとよい。

前提知識:
- ECS のデータ置き場（Component / Resource の違い） → [[20260716_bevy-resource-res-resmut-basics]]

この基礎知識を使う実装側のノート:
- ブロックの動的メッシュ構築の詳細（`Mesh::ATTRIBUTE_*` / インデックス / 扇形三角形分割）→ [[20260725_bevy-dynamic-mesh-build-brick-mesh]]
- 破れた辺を中点変位法でギザギザにする実装全体 → [[20260725_brick-torn-edge-midpoint-displacement]]
- Sprite の `rect` で画像を切り出す方式（移行前の姿。UV と考え方が対応する） → [[20260715_brick-image-rendering]]

---

## 1. Mesh は「針金の型紙」

`Mesh` は **位置（頂点）だけを持つ、色も画像も一切無い透明な骨組み**。針金で四角形の輪郭を
作ったところを想像するとよい。針金の型紙だけでは、そこに何も貼っていないので何も見えない
（画面に描く色の情報を Mesh 自体は持っていない）。

`game_engine/src/rendering.rs` の `build_brick_mesh` が組み立てているのはまさにこれで、頂点の
座標（`positions`）を積んでいく処理は色や画像の話を一切していない（3 節で詳しく見る）。

## 2. Material（`ColorMaterial`）は別物の「実際の写真・色データ」

`Material`（このプロジェクトでは `ColorMaterial`）は Mesh とは **完全に独立した別のデータ**。
単色そのものか、貼りたい画像のハンドルを持っているだけの「実際の写真・色データ」であって、
単体では「どこに貼るか（=どの形の上に乗せるか）」を一切知らない。

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

`ColorMaterial::from(*color)` は単色、`ColorMaterial { texture: Some(...), .. }` は画像ハンドルを
持つだけの箱。どちらも「形」の情報は持っていない。

## 3. UV座標は「画像の中の場所を0〜1の2つの数字（横%, 縦%）で表したもの」

UV座標とは、画像の中のどの位置かを **横%・縦%の2つの数字（それぞれ0〜1の範囲）** で表したもの。

- UV = `(0, 0)` → 画像の左上
- UV = `(1, 1)` → 画像の右下
- UV = `(0.5, 0.5)` → 画像の真ん中

たとえば UV = `(0.25, 0.8)` なら「横方向に25%進んだところ、縦方向に80%進んだところ」の色を
指す、というだけの意味。

**重要な注意点**: UVの数字自体は画像の中身を全く持っていない。あくまで Mesh 側の各頂点に
「この頂点は画像の何%・何%の位置に対応する」と書かれた **空っぽの付箋** のような数字に過ぎない。
Mesh に UV を持たせても、それだけでは Mesh は依然として色を持たない透明な型紙のまま
（1節の「針金」という説明は嘘ではない。UVは針金に貼った付箋のメモであって、写真そのものでは
ない、ということ）。実際に色が出るのは、この付箋の数字を頼りに Material（写真）から色を
持ってくる処理（4節）が走るから。

`vertex_uv`（`rendering.rs:148`）が、ブロックのローカル座標をこの0〜1のUVへ変換している:

```rust
// game_engine/src/rendering.rs:148
fn vertex_uv(p: Vec2, size: Vec2, fill: &BrickFill) -> [f32; 2] {
    match fill {
        BrickFill::Color(_) => [0.0, 0.0],
        BrickFill::Textured { uv_rect, .. } => {
            let t = Vec2::new(p.x / size.x + 0.5, 0.5 - p.y / size.y);
            let uv = uv_rect.min + t * uv_rect.size();
            [uv.x, uv.y]
        }
    }
}
```

`t` がまず `p` をブロック内での 0〜1 の相対位置（横%・縦%）に変換し、`uv_rect`（画像内の
切り出し矩形。0〜1で正規化済み）の範囲にマッピングし直している。`BrickFill::Color` の場合は
そもそも画像を使わないので `[0.0, 0.0]`（値自体は使われない）。

## 4. MeshとMaterialは別物だが、両方を同じentityに乗せると自動で色が塗られる

Mesh（針金の型紙）と Material（写真）は完全に別のデータだが、**両方を同じ entity に乗せる**と、
Bevy の描画プラグイン（`Material2dPlugin<ColorMaterial>`。`DefaultPlugins` に含まれるので
明示的な追加は不要）が毎フレーム自動でこれをやってくれる:

1. 頂点ごとの UV（付箋の数字）を見る
2. その UV が指す「横%・縦%」の位置の色を Material（写真）から取ってくる
3. その色を、その頂点の位置（Mesh側の座標）に塗る

これが「Mesh だけでは画像は自動で出ない」の答え。Mesh 単独では色の出しどころが無く、
Material 単独では貼る形が無い。両方が揃って、かつ **同じ entity に乗っている**ときだけ、
描画プラグインが両者を結びつけて実際の見た目を作る。

```rust
// game_engine/src/rendering.rs:107
commands.spawn((
    Mesh2d(brick_assets.meshes.add(mesh)),
    MeshMaterial2d(brick_assets.materials.add(material)),
    Transform::from_translation(position.extend(0.0)),
    Brick { size, cell, fill },
    Collider,
    broken,
));
```

`Mesh2d(...)` と `MeshMaterial2d(...)` という2つのコンポーネントが同じ `commands.spawn` の
タプルに入っている＝同じ entity に乗っている、という点がポイント。

## 5. `positions` と `uvs` は1対1に対応した配列

`build_brick_mesh` の中で、頂点の位置（`positions`）とUV（`uvs`）は**同じ添字が同じ頂点を指す
平行配列**として積まれる。`positions[i]` という頂点があれば、`uvs[i]` は必ずその同じ頂点のUV。

```rust
// game_engine/src/rendering.rs:187
let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n + 1);
let mut normals: Vec<[f32; 3]> = Vec::with_capacity(n + 1);
let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(n + 1);

positions.push([0.0, 0.0, 0.0]);
normals.push([0.0, 0.0, 1.0]);
uvs.push(vertex_uv(Vec2::ZERO, size, fill));

for p in &boundary {
    positions.push([p.x, p.y, 0.0]);
    normals.push([0.0, 0.0, 1.0]);
    uvs.push(vertex_uv(*p, size, fill));
}
```

`positions.push(...)` と `uvs.push(vertex_uv(*p, size, fill))` が常に対で積まれているのが見える
通り、「この頂点の位置」を積んだら必ず同じタイミングで「この頂点のUV」も積む、というルールを
コードで徹底している。これを守らないと、添字がずれて「別の頂点のUV」を誤って使うことになり、
画像がねじれて貼られる。詳しいメッシュ構築（インデックス・扇形三角形分割）は
[[20260725_bevy-dynamic-mesh-build-brick-mesh]] を参照。

## 6. 「中点」という言葉の2つの別の意味（混同しやすい注意点）

`build_brick_mesh` を読むと「中点」という言葉が2箇所に出てくるが、これは**互いに無関係な
2つの別の話**が、たまたま同じ「中点」という日本語を使っているだけ。

### ①ブロックの真ん中の点（扇形三角形分割の支点）

```rust
// game_engine/src/rendering.rs:191
positions.push([0.0, 0.0, 0.0]);
```

これはブロックの **真ん中の点** `(0.0, 0.0, 0.0)`。3節で見た通り、Mesh はこの中心点と輪郭上の
点を結んで三角形を作る（扇形三角形分割 = fan triangulation）ときの**支点として使うだけ**の点。
中点変位法とは無関係。

### ②「中点変位法（midpoint displacement）」の中点（辺の中点）

こちらは `game_engine/src/tear.rs` の `midpoint_displace` が使う「中点」で、①とは全く別の対象を
指す。

```rust
// game_engine/src/tear.rs:47
fn midpoint_displace(a: Vec2, b: Vec2, depth: u32, roughness: f32, rng: &mut TearRng, out: &mut Vec<Vec2>) {
    if depth == 0 {
        return;
    }
    let mid = (a + b) / 2.0;
    let edge = b - a;
    let normal = Vec2::new(-edge.y, edge.x).normalize_or_zero();
    let amplitude = edge.length() * roughness;
    let offset = (rng.next_unit() * 2.0 - 1.0) * amplitude;
    let displaced = mid + normal * offset;

    midpoint_displace(a, displaced, depth - 1, roughness, rng, out);
    out.push(displaced);
    midpoint_displace(displaced, b, depth - 1, roughness, rng, out);
}
```

ここでの「中点」（`mid`）は、ブロックの真ん中の点ではなく、**辺（角と角の間）の中点**、
つまり `a` と `b` という2つの端点を結ぶ辺のちょうど真ん中の位置。この中点を辺の法線方向へ
ランダムに（`offset`）ずらして `displaced` を作り、ギザギザの変位点として輪郭に差し込む
（`a`→`displaced`、`displaced`→`b` へ再帰していくことで分割が細かくなる）。

つまり①は「ブロック全体の中心」、②は「辺1本の中点」で、対象そのものが違う。①は扇形三角形
分割の支点として固定的に使われるだけの点、②はギザギザ生成アルゴリズムの名前の由来になっている
「動く点」。両者に処理上の関係は無い。実装の詳細は [[20260725_brick-torn-edge-midpoint-displacement]]
を参照。

## 7. 頂点の3つの属性：`ATTRIBUTE_POSITION` / `ATTRIBUTE_NORMAL` / `ATTRIBUTE_UV_0`

`build_brick_mesh` が各頂点に持たせる情報は3種類ある。

| 属性 | 型 | 中身 |
|---|---|---|
| `Mesh::ATTRIBUTE_POSITION` | `[f32; 3]` | その頂点のローカル座標（1節の「針金」の形そのもの） |
| `Mesh::ATTRIBUTE_NORMAL` | `[f32; 3]` | 法線（面がどちら向きを向いているか） |
| `Mesh::ATTRIBUTE_UV_0` | `[f32; 2]` | 3節で見た0〜1のUV（画像の何%・何%か） |

`ATTRIBUTE_NORMAL` は3Dで面の向きを表すために本来必要な情報だが、このプロジェクトは2D平面
（すべての頂点が同一平面上）なので、全頂点で `[0.0, 0.0, 1.0]`（画面手前向き）固定という
**形式的な値に過ぎない**。実際に向きが変わることは無い。

```rust
// game_engine/src/rendering.rs:187-193（抜粋）
normals.push([0.0, 0.0, 1.0]);
```

## 8. まとめ

- Mesh = 色も画像も持たない、位置だけの透明な骨組み（針金の型紙）。
- Material（`ColorMaterial`）= Mesh とは独立した、実際の色・画像データ。単体では貼り先を知らない。
- UV座標 = 画像内の位置を横%・縦%（0〜1）で表した数字。数字自体は画像の中身を持たない付箋。
- Mesh と Material を同じ entity に乗せると、Bevy の描画プラグインが UV を見て Material から
  色を取ってきて塗る、という処理を毎フレーム自動でやる。
- `positions[i]` と `uvs[i]` は同じ頂点を指す1対1の平行配列。
- 「中点」は①ブロック全体の中心（扇の支点）と②辺の中点（中点変位法）の2つの無関係な意味がある。
- `ATTRIBUTE_NORMAL` は2D平面では全頂点 `[0,0,1]` 固定の形式的な値。

## 9. 関連ファイル

- `game_engine/src/rendering.rs` — `build_brick_mesh` / `vertex_uv` / `build_brick_material` / `spawn_brick`
- `game_engine/src/tear.rs` — `midpoint_displace`（辺の中点変位。①とは別の「中点」）
