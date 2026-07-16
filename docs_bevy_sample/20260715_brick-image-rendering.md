# ブロックを画像で描画する（盤面に貼った 1 枚の絵を各ブロックが切り出す＝リビール方式）

日付: 2026-07-15

ブロックの見た目を単色から **画像** に変えた実装の記録。ポイントは「画像を各ブロックに
引き伸ばして貼る（テクスチャ漬け）」のではなく、**盤面全体に 1 枚の絵を貼ったとみなし、
各ブロックはその絵のうち自分が覆う領域だけを切り出して表示する**こと。全ブロックが揃うと
1 枚の絵になり、ブロックを壊すとその穴から背後の背景画像が覗く。

画像の受け渡し（React → Bevy）は背景画像と同じ注入パターンを踏襲しているので、前提は
[[20260711_react-to-bevy-init-params]] / [[20260711_react-to-bevy-background-injection]] を参照。
アスペクト比維持・黒余白の扱いは [[20260715_aspect-ratio-and-letterbox]]、コードの配置
（モジュール分割）は [[20260715_main-rs-module-split]] を参照。

---

## 1. 何を作ったか（要件の変遷）

要件は会話の中で段階的に固まった。誤解しやすいので経緯ごと残す:

1. 「ブロックのレンダリングを画像にしたい」
2. 「テクスチャ漬け（＝画像をセルに引き伸ばし）はダメ。**そのまま**貼りたい」
3. 「**ブロックがあるところだけその画像が写り、無ければ背景画像が映る**」
   → これが決定版の仕様。画像は盤面に 1 枚貼られていて、ブロックが *窓* のように
   その一部分を見せる。ブロックを壊すと窓が消え、背後の背景が見える。
4. 画像は当初「2 種類（配列）」だったが、最終的に **単数（`brickImage: string`）** に統一。
   → 交互配置（モザイク）は不要。1 枚の絵をきれいにリビールするのが目的だったため。

**「画像 2 種類」の最終的な意味** = 背景画像 1 枚 + ブロック画像 1 枚。ブロック画像が
前面、背景画像が背後で、壊すと背景が覗く。

## 2. データフロー

背景注入と同じ「郵便受け」方式（`window.__BREAKOUT_CONFIG__`）に相乗り:

```
App.tsx (brickImage={URL})
  ↓
BevyGame.tsx: fetch(URL) → bytes 化 → config.brickImage = { bytes, mime }
  ↓
window.__BREAKOUT_CONFIG__ = config      ← init() より前に投函
  ↓
main.rs: injected_brick_image() → Option<Image>（デコード）
  ↓
.insert_resource(BrickImageOverride(...))
  ↓
setup(): 画像を Assets<Image> に登録し、全ブロックが共有。各ブロックは自分の領域を切り出す
```

JS ↔ Rust の契約（`brickImage`）:

```ts
window.__BREAKOUT_CONFIG__ = {
  // ...背景・配置と同居
  brickImage?: { bytes: Uint8Array; mime?: string }, // fetch 済み画像のバイト列 + content-type
};
```

`brickImage` が無い / fetch 失敗 / デコード失敗 のいずれでも、Bevy 側は **単色ブロック**
（`BRICK_COLOR`）にフォールバックする（致命ではない）。ネイティブビルドでは注入が無いので常に単色。

## 3. 実装の核心：Sprite の `rect` で「切り出し」を表現する

Bevy の `Sprite` は次の 3 つを組み合わせて、画像の一部分を任意サイズで描ける:

| フィールド | 役割 |
|---|---|
| `image` | 貼る画像ハンドル |
| `rect: Option<Rect>` | 画像内の **切り出し領域**（ピクセル単位） |
| `custom_size: Option<Vec2>` | 描画時のサイズ（`rect` の中身をここに収める） |

### なぜ `custom_size = Some(Vec2::ONE)` なのか

このプロジェクトのブロックは **`Transform.scale` にセルサイズを入れて 1x1 を拡大**する方式
（壁・パドルと共通）。しかも当たり判定が `Transform.scale` を使う（後述）ので、scale は
セルサイズのまま動かせない。

- `custom_size` を未指定にすると、画像スプライトは **画像本来のピクセル寸法** が基準サイズになり、
  そこに `scale`（セルサイズ）が掛かって巨大化してしまう。
- そこで `custom_size: Some(Vec2::ONE)` で基準を 1x1 に固定 → `rect` の中身を 1x1 に収める →
  `Transform.scale = セルサイズ` で正しくセルの大きさに拡大される。

```rust
// rendering.rs（要点）
let sprite = match image {
    Some((handle, image_size)) => match brick_image_rect(position, size, image_size) {
        Some(rect) => Sprite {
            image: handle,
            custom_size: Some(Vec2::ONE), // 基準 1x1 → Transform.scale でセルサイズへ
            rect: Some(rect),             // 画像内の切り出し領域
            ..default()
        },
        None => Sprite { color: Color::BLACK, ..default() }, // 領域外は黒（[[20260715_aspect-ratio-and-letterbox]]）
    },
    None => Sprite { color: BRICK_COLOR, ..default() },      // 画像未指定は単色
};
commands.spawn((sprite, Transform { translation: position.extend(0.0), scale: size.extend(1.0), .. }, Brick, Collider));
```

## 4. 当たり判定との両立（ハマりどころ）

衝突判定は `Transform.scale` からブロックの AABB を作る:

```rust
// systems.rs check_for_collisions
Aabb2d::new(collider_transform.translation.truncate(), collider_transform.scale.truncate() / 2.)
```

### AABB とは（前提知識）

**AABB = Axis-Aligned Bounding Box（軸並行境界ボックス）**。オブジェクトを「**傾いていない
長方形**（辺が必ず X 軸・Y 軸に平行）」で囲む、当たり判定の定番テクニック。斜めに回転しない
という制約を課すことで、衝突判定が比較演算だけで済む＝激速になるのが狙い。

```
A と B が重なる
 ⇔ (A左 < B右) かつ (A右 > B左)   ← X 方向で重なる
   かつ (A下 < B上) かつ (A上 > B下) ← Y 方向で重なる
```

三角関数も平方根も要らないので、「ボール vs 大量のブロック」を毎フレーム総当たりする
ブロック崩しにぴったり。ブロックは回転しないので、回転を許す OBB（Oriented Bounding Box）
のような重い判定は不要。

`Aabb2d` は「**中心 + 半サイズ**」で箱を表す:

```rust
Aabb2d::new(
    collider_transform.translation.truncate(), // 中心座標(x,y)。Vec3 から z を捨てて Vec2 に
    collider_transform.scale.truncate() / 2.,   // 中心から縁までの距離 = 実サイズ / 2
)
```

`scale.truncate() / 2.` で `/ 2.` しているのは、`Aabb2d` が半サイズを取る形式だから。そして
ここが成り立つのは **元のスプライト/メッシュが 1×1 単位で作られていて、`scale` がそのまま
実サイズ（ピクセル）になる**という前提のおかげ（[3](#3-実装の核心sprite-の-rect-で切り出しを表現する) の `custom_size = Vec2::ONE` と対）。

### なぜ「ハマりどころ」なのか

つまり **見た目のために scale を触ると当たり判定まで変わる**。だから画像ブロックでも
`Transform.scale = セルサイズ` を維持し、サイズ調整は `custom_size`（1x1）と `rect` 側で行う。
この分離が今回の設計の肝。

## 5. ワールド座標 → 画像ピクセルの写像（`brick_image_rect`）

各ブロックが画像のどこを切り出すかは、ブロックの中心 `position` とセル `size` から計算する。
アリーナ（`x∈[LEFT_WALL,RIGHT_WALL], y∈[BOTTOM_WALL,TOP_WALL]`）に画像を敷いたとみなし、
ブロックが覆うワールド矩形を画像ピクセル矩形へ変換する。

注意点:

- **y 軸の向きが逆**。ワールドは y 上向き、画像は y 下向き。ワールド上端 `TOP_WALL`（や内接矩形の
  上端）を画像の `v=0` に対応させ、v は上下反転して計算する。
- 実際には「引き伸ばさない（アスペクト比維持）」ため、盤面全体ではなく **画像を contain
  フィットさせた内接矩形** に対して写像する。内接矩形からはみ出すブロックは `None` を返して
  黒く塗る。詳細は [[20260715_aspect-ratio-and-letterbox]]。

```rust
// rendering.rs（アスペクト比維持版・要点）
fn brick_image_rect(position: Vec2, size: Vec2, image_size: Vec2) -> Option<Rect> {
    let field = Vec2::new(RIGHT_WALL - LEFT_WALL, TOP_WALL - BOTTOM_WALL);
    let display = contain_fit(image_size, field); // 比率維持で内接させた表示寸法
    let half = display / 2.0;                      // 中心原点なので [-half, half]
    let (left, right) = (position.x - size.x/2.0, position.x + size.x/2.0);
    let (top, bottom) = (position.y + size.y/2.0, position.y - size.y/2.0);
    if left < -half.x || right > half.x || bottom < -half.y || top > half.y {
        return None; // 内接矩形の外 → 黒ブロック
    }
    let u_min = (left  + half.x) / display.x * image_size.x;
    let u_max = (right + half.x) / display.x * image_size.x;
    let v_min = (half.y - top)    / display.y * image_size.y; // y 反転
    let v_max = (half.y - bottom) / display.y * image_size.y;
    Some(Rect::new(u_min, v_min, u_max, v_max))
}
```

## 6. 重ね順（z）

- 背景スプライト: `z = -10`（最背面）。
- ブロック / 壁 / パドル: `z = 0`。
- ボール: `z = 1`（最前面）。

ブロック（z=0・不透明）が背景（z=-10）を覆い、破壊で despawn されるとその位置に背景が見える。
これが「壊すと背景が覗く」を実現している。

## 7. 関連ファイル

- `frontend/src/components/BevyGame.tsx` … `brickImage` prop を fetch → `config.brickImage`
- `frontend/src/App.tsx` … `brickImage={BRICK_IMAGE_URL}`
- `game_engine/src/injection.rs` … `BrickImageOverride` / `injected_brick_image()`
- `game_engine/src/rendering.rs` … `spawn_brick` / `brick_image_rect` / `contain_fit`
- `game_engine/src/setup.rs` … 画像を Assets に登録し全ブロックで共有
