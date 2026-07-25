# ブロックの「破れた辺」を中点変位法でギザギザに描く

日付: 2026-07-25

ブロックが破壊されて隣が空洞になった辺だけ、中点変位法(midpoint displacement)でギザギザの
破れた境界として再描画する機能の実装記録。単色ブロックのときも画像ブロックのときも同じ仕組みで
効く。ブロックの描画は本実装で Sprite の矩形描画から Mesh2d(動的メッシュ) + ColorMaterial に
移行している。

前提知識:
- Mesh・Material・UV座標とは何か（Mesh2d + MeshMaterial2d<ColorMaterial> の基本） → [[20260725_bevy-mesh-material-uv-basics]]
- ブロックを画像で切り出して描く方式（本実装が置き換える前の姿） → [[20260715_brick-image-rendering]]
- 衝突判定システム `check_for_collisions` の読み方 → [[20260723_check-for-collisions-system]]
- `Assets<T>` / `Handle<T>` に登録して使う定石（`BrickAssets` の `meshes.add` / `materials.add`）→ [[20260723_bevy-assets-handle-add-pattern]]
- Bevy(WASM) からフロントへの通知の仕組み（Event/observer の使い方の類例） → [[20260717_bevy-to-frontend-event-notification]]
- クエリ借用競合と `Without<Ball>`（`check_for_collisions` の別クエリとの排他） → [[20260723_bevy-b0001-query-conflict-without]]

---

## 1. 何を作ったか

ブロックが壊れると、**かつてそのブロックと接していた隣接ブロックの、接触面だった辺だけ**が
直線ではなくギザギザの破れた輪郭になる。壁際やレイアウト上の隙間に面している辺（元々隣に
ブロックが存在しなかった辺）は対象外で、常に直線のまま。

コンポーネントの分割は **可変性・変更検知** を軸に決めた。spawn 時に確定してその後変わらない
不変データ（大きさ・格子座標・塗り方）は `Brick` 構造体に 1 つにまとめ、実行中に変化して
`Changed<T>` で絞り込み再描画したい「破れているか」だけを `BrokenEdges` という独立コンポーネント
に切り出している（CLAUDE.md 更新後の方針）。

補足: CLAUDE.md の「役割が違うものは別の型に分ける」はあくまで **振る舞いを判別するマーカー**
（`Option<&T>` / `With<T>` で分岐する `Brick` / `DeathZone` など）への指針であって、同じ 1
エンティティに常に同居し粒度も同じ、判別にも使わない不変データまでフィールドごとに別コンポーネント
へ割るのは過剰分割になる。だから `size` / `cell` / `fill` は `Brick` に集約し、変更検知が要る
`BrokenEdges` だけを分けている。

## 2. 型設計（`components.rs`）

不変データ（大きさ・格子座標・塗り方）は `Brick` 構造体に集約している。`fill` が `Handle<Image>`
を含むため `Copy` にはできず `Clone` のみ。

```rust
// game_engine/src/components.rs:48
#[derive(Component, Clone)]
pub struct Brick {
    pub size: Vec2,
    pub cell: BrickCell,
    pub fill: BrickFill,
}
```

`Brick` は旧マーカー（フィールド無し）から、`size` / `cell` / `fill` の 3 つの不変データを持つ形に
変わった。ブロックは Sprite ではなく動的メッシュで描くため `Transform.scale` を大きさとして使わず、
`Brick.size` がメッシュ寸法そのもの（3 節・5 節参照）。

`cell`（格子座標）と `fill`（塗り方）は **`Brick` のフィールド用の値型**であり、それ単独では
コンポーネントにしない（`#[derive(Component)]` を付けない）。

```rust
// game_engine/src/components.rs:61
// 値型（Component derive 無し）。Brick.cell として持つ
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BrickCell {
    pub row: i32,
    pub col: i32,
}
```

ブロックの盤面上の格子座標。隣接ブロックが破壊されたとき「自分から見てどの辺が破れるか」を
row/col の差分だけで判定する基準になる（6 節）。

```rust
// game_engine/src/components.rs:81
// 値型（Component derive 無し）。Brick.fill として持つ
#[derive(Clone)]
pub enum BrickFill {
    Textured { image: Handle<Image>, uv_rect: Rect },
    Color(Color),
}
```

ブロックの塗り方。壊れた辺の再描画（メッシュ再構築）でも Sprite 時代と同じ見た目の規則
（画像切り出し優先、範囲外は黒、指定無しは単色）を再現するために保持する。

一方、実行中に変化する「破れた辺」だけは **独立コンポーネント** `BrokenEdges` として分けてある
（`Changed<BrokenEdges>` で絞り込み再描画するため。分割の軸は「役割」ではなく「可変性・変更検知」）。

```rust
// game_engine/src/components.rs:72
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BrokenEdges {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}
```

上下左右それぞれの辺が「破れた境界」かどうか。true の辺だけ中点変位法のギザギザで再描画する。
一度も隣接ブロックが存在しなかった辺はここに反映されない設計（コメントに明記）。

```rust
// game_engine/src/components.rs:79
#[derive(Event)]
pub struct BrickDestroyed {
    pub cell: BrickCell,
}
```

ブロックが破壊された直後に発火する内部イベント。`mark_broken_edges_on_brick_destroyed`
（`systems.rs`）がこれを拾って隣接ブロックへ「破れた境界」を伝える。

## 3. `injection.rs`：`BrickLayout` への `cells` 追加

```rust
// game_engine/src/injection.rs:102
pub struct BrickLayout {
    pub positions: Vec<Vec2>,
    pub cell_size: Vec2,
    pub cells: Vec<BrickCell>,
}
```

破れ判定・ギザギザの種（シード）の両方に `BrickCell`（行・列）が要るため、`BrickLayout` に
`cells` を追加した。算出方法はデフォルト配置と JS 注入配置で異なる。

- **デフォルト配置**（`default_brick_layout`, `injection.rs:112`）: アリーナを敷き詰める
  `for row in 0..n_rows { for column in 0..n_columns { ... } }` の二重ループそのものが
  row/column を持っているので、`cells.push(BrickCell { row: row as i32, col: column as i32 })`
  とループ変数をそのまま使うだけで済む（`injection.rs:143-146`）。
- **JS 注入配置**（`injected_brick_layout`, `injection.rs:168`）: JS から渡るのはワールド座標
  の `positions`（`{x, y}` の配列）だけで、行列座標は付いてこない。そこで
  「JS 側の座標は必ずしも 0 始まりではないが格子には整合している」ことを前提に、
  全ブロックの `x`/`y` の最小値を格子の原点とみなし、各ブロックの位置をそこからの相対位置
  として `cell_size` で割って丸めることで行列座標を逆算する（`injection.rs:215-231`）:

```rust
// game_engine/src/injection.rs:217
let origin_x = positions.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
let origin_y = positions.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
let cells = positions
    .iter()
    .map(|p| BrickCell {
        row: ((p.y - origin_y) / cell_size.y).round() as i32,
        col: ((p.x - origin_x) / cell_size.x).round() as i32,
    })
    .collect();
```

## 4. `rendering.rs`：Sprite から Mesh2d + ColorMaterial への移行

```rust
// game_engine/src/rendering.rs:7
//! ブロックは Sprite ではなく Mesh2d(動的メッシュ) + ColorMaterial で描画する。壁・パドルと違い
//! ブロックは破壊された隣との接触面だけを中点変位法のギザギザ輪郭に再構築する必要があり、
//! それには頂点を自前で持てるメッシュが要る。
```

Sprite は矩形（あるいは `rect` で切り出した矩形）しか描けないため、辺の一部だけを凹凸のある
輪郭に変形させることができない。ブロックだけメッシュ描画に切り替え、壁・パドル・背景は従来の
Sprite のままにしている（役割が変わらないものは変えない）。

`BrickAssets`（`rendering.rs:27`）は `meshes: ResMut<Assets<Mesh>>` と
`materials: ResMut<Assets<ColorMaterial>>` を束ねた `SystemParam`。`spawn_brick` を呼ぶ
`setup`・`reset_game` の両方で常にセットで必要になり、バラのまま渡すと他の必須パラメータと
合わせて `clippy::too_many_arguments` を誘発するため、型としてまとめている。

### 4.1 `build_brick_mesh`：中心からの扇形三角形分割(fan triangulation)

```rust
// game_engine/src/rendering.rs:163
fn build_brick_mesh(size: Vec2, cell: BrickCell, broken: &BrokenEdges, fill: &BrickFill) -> Mesh
```

ローカル原点 `(0,0)` 中心、四隅 `(±size.x/2, ±size.y/2)` の矩形を輪郭の出発点とし、下→右→上→左
（反時計回り）に走査する。`edge_broken[i]` が true の辺だけ、区間の始点・終点の間に
`crate::tear::push_torn_edge`（`tear.rs`）で変位点を差し込んで輪郭 (`boundary: Vec<Vec2>`) に積む。
中点変位法の実行部（乱数・シード・再帰）は `tear` モジュールに分離されており、`build_brick_mesh`
からはこの 1 行を呼ぶだけ（詳細は [[20260725_bevy-dynamic-mesh-build-brick-mesh]]）。

メッシュは「中心 `(0,0)` を追加した扇形三角形分割」で作る（`positions` の先頭が中心、以降が
境界点）。これは実装が単純な反面、**輪郭が常に中心から見える形（star-shaped）であること**が
前提になる。輪郭のどこかが中心から見て他の点の背後に回り込む（＝角度が後退する）と、扇の三角形
が自己交差してメッシュが破綻する。この制約が `TEAR_ROUGHNESS` の安全域を決めている（7 節）。

この保証はテストで回帰的に固定化されている（`rendering.rs:230`
`build_brick_mesh_is_star_shaped_for_many_cells` / `build_brick_mesh_is_star_shaped_for_partial_breaks`）。
境界点を中心から見た角度が輪郭を 1 周する間単調増加であることを確認する
`assert_star_shaped`（`rendering.rs:260`）で判定している。

### 4.2 中点変位法の実行部は `tear.rs` に分離

中点変位のギザギザ生成部は `rendering.rs` から新モジュール `src/tear.rs` へ切り出してある。
`rendering.rs` は入口 `push_torn_edge` を呼ぶだけで、乱数（`TearRng`）・シード（`seed_for`）・
再帰（`midpoint_displace`）と `TEAR_DEPTH` / `TEAR_ROUGHNESS` の使用は `tear.rs` に隠蔽される
（この分離リファクタと Mesh 構築の詳細は [[20260725_bevy-dynamic-mesh-build-brick-mesh]]）。

```rust
// game_engine/src/tear.rs:67
pub fn push_torn_edge(cell: BrickCell, edge_index: u32, start: Vec2, end: Vec2, out: &mut Vec<Vec2>) {
    let mut rng = TearRng::new(seed_for(cell, edge_index));
    midpoint_displace(start, end, TEAR_DEPTH, TEAR_ROUGHNESS, &mut rng, out);
}
```

- `midpoint_displace`（`tear.rs:47`）: `a`→`b` の辺の中点を辺の法線方向へランダム変位させ、
  `a`→変位点・変位点→`b` の 2 区間へ再帰する。両端 `a`/`b` は積まない（呼び出し側が管理）。
  再帰 1 段ごとに辺長が半分になり `amplitude = edge.length() * roughness` も自動減衰。
  `depth` 段の再帰で辺は `2^depth` 分割される。`midpoint_displace` と `TearRng` は private。
- `TearRng`（`tear.rs:13`）: xorshift32 の自作 PRNG（`next_u32`/`next_unit`）。依存クレートを
  増やさず自作にしたのは、wasm32 で `rand` 等が要求する `getrandom` の feature-flag 対応を
  避けつつ、見た目の再現性（同じブロック・同じ辺は毎回同じ形）を確保するため。
- `seed_for`（`tear.rs:35`, テストのため `pub`）: `BrickCell`（row/col）と辺番号
  （`edge_index`: 下=0, 右=1, 上=2, 左=3）から決定的な種を作る:

```rust
// game_engine/src/tear.rs:38
r.wrapping_mul(73856093)
    ^ c.wrapping_mul(19349663)
    ^ edge_index.wrapping_mul(83492791)
    ^ 0x9E3779B9
```

同じセル・同じ辺なら常に同じ種になり、同じギザギザが再現される（決定性テスト
`seed_for_is_deterministic_per_cell_and_edge` は `tear.rs:78` に移設済み）。

### 4.3 `redraw_broken_bricks`：差分だけメッシュ再構築

```rust
// game_engine/src/rendering.rs:122
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

不変データを `Brick` に集約したため、クエリは `&Brick` と `&BrokenEdges` の 2 コンポーネントだけで
済む（旧 `&BrickCell` / `&BrickFill` は `Brick.cell` / `Brick.fill` から取る）。

`Changed<BrokenEdges>` フィルタにより、`BrokenEdges` が更新された（＝隣が新たに破壊された）
ブロックだけを対象にメッシュを再構築する。毎フレーム全ブロックのメッシュを作り直すコストを
避けている。

## 5. `systems.rs`：当たり判定サイズと破壊イベント

### 5.1 ブロックだけ `Brick.size` を当たり判定サイズの基準にする

```rust
// game_engine/src/systems.rs:164
let half_extents = match maybe_brick {
    Some(brick) => brick.size / 2.0,
    None => collider_transform.scale.truncate() / 2.0,
};
let collision = ball_collision(
    BoundingCircle::new(ball_transform.translation.truncate(), BALL_DIAMETER / 2.),
    Aabb2d::new(collider_transform.translation.truncate(), half_extents),
);
```

ブロックは `Transform.scale` を大きさとして使わず、`Brick.size` をメッシュ寸法として直接
保持している（4 節）ので、当たり判定の半サイズもそこから取る必要がある。壁・パドル・DeathZone
は従来通り `Transform.scale` 基準のまま（[[20260723_check-for-collisions-system]] 参照）。
`collider_query` は `Query<(Entity, &Transform, Option<&Brick>, Option<&DeathZone>), ...>`
（`systems.rs:145`）。不変データを `Brick` に集約したので別途 `Option<&BrickCell>` は要らず、
破壊時のセルは `brick.cell` から取り出す。

### 5.2 破壊時に `BrickDestroyed` を発火

```rust
// game_engine/src/systems.rs:194
if let Some(brick) = maybe_brick {
    commands.entity(collider_entity).despawn();
    score.0 += 1;
    commands.trigger(BrickDestroyed { cell: brick.cell });
}
```

破壊セルは `Brick.cell` から直接取れるので、`maybe_brick` が `Some` なら無条件で
`BrickDestroyed` を発火できる（旧 `Option<&BrickCell>` の有無チェックは不要になった）。

### 5.3 `mark_broken_edges_on_brick_destroyed`：隣接判定は行列座標の差分だけ

```rust
// game_engine/src/systems.rs:270
pub fn mark_broken_edges_on_brick_destroyed(
    trigger: On<BrickDestroyed>,
    mut bricks: Query<(&Brick, &mut BrokenEdges)>,
) {
    let destroyed = trigger.cell;
    for (brick, mut broken) in &mut bricks {
        let cell = brick.cell;
        if cell.row == destroyed.row + 1 && cell.col == destroyed.col {
            broken.bottom = true; // 自分は破壊されたセルの真上 → 自分の下辺が破れる
        } else if cell.row == destroyed.row - 1 && cell.col == destroyed.col {
            broken.top = true; // 真下 → 上辺が破れる
        } else if cell.col == destroyed.col + 1 && cell.row == destroyed.row {
            broken.left = true; // 右隣 → 左辺が破れる
        } else if cell.col == destroyed.col - 1 && cell.row == destroyed.row {
            broken.right = true; // 左隣 → 右辺が破れる
        }
    }
}
```

`BrickDestroyed` observer は、生存中の全ブロックを回して行列座標の差分（±1、同じ row/col）だけ
を見る。斜めに隣接するだけ（差分が row・col 両方±1）は辺を共有しないので対象外
（`systems.rs:391` のテストで確認）。

**この経路が「かつて隣接ブロックがあった辺だけ破れる」という要求を保証する仕組み**: `BrokenEdges`
を立てる唯一の経路がこの observer であり、observer が動くのはブロックが実際に破壊されて
`BrickDestroyed` が飛んだときだけ。壁際やレイアウト上の隙間に面した辺は、そこに元からブロックが
存在しないため `BrickDestroyed` の発火元にもなり得ず、この経路を絶対に通らない。つまり
「破れる」という状態変化の入口を 1 本に絞ることで、意図しない辺のギザギザ化を構造的に防いでいる。

テスト `marks_only_the_edge_facing_the_destroyed_neighbor`（`systems.rs:349`）で
上下左右・非隣接・斜め隣接の 6 パターンを固定化している。

## 6. `config.rs`：調整可能パラメータ

```rust
// game_engine/src/config.rs:62
pub const TEAR_DEPTH: u32 = 2;
pub const TEAR_ROUGHNESS: f32 = 0.45;
```

| パラメータ | 意味 | 効果 |
|---|---|---|
| `TEAR_DEPTH` | 中点変位の再帰段数 | 辺が `2^depth` 分割される。大きいほどギザギザが細かくなる |
| `TEAR_ROUGHNESS` | 振れ幅の辺長に対する比率 | 大きいほどギザギザの振れが大きくなる |

**安全域の制約**: 4.1 節の通り `build_brick_mesh` は中心からの扇形三角形分割を使うため、輪郭は
常に中心から見える(star-shaped)範囲に収める必要がある。既定のブロック寸法（`BRICK_SIZE =
Vec2::new(50., 30.)`、`config.rs:31`）に対して振れ幅がその半分に対して大きすぎると、輪郭が
中心から見えなくなり star-shaped 制約が破れ、メッシュが自己交差する。安全域の**唯一の権威は
回帰テスト**であって固定のしきい値ではない: テストがブロック寸法と `TEAR_ROUGHNESS` 定数を
そのまま使って全セル・全辺組み合わせを検証しているため、現行値でテストが green なら安全域内。
現行の `TEAR_ROUGHNESS = 0.45` はテスト `build_brick_mesh_is_star_shaped_for_*`
（`rendering.rs:283`, `rendering.rs:296`）が green であることを 2026-07-25 に確認済み。
（過去に「固定の目安 0.3 程度まで」と書いていたが、実測で 0.45 でも破綻しないため撤回した。）

## 7. 設計判断の要点（まとめ）

- **分割の軸は「可変性・変更検知」**: spawn 時に確定する不変データ（`size` / `cell` / `fill`）は
  `Brick` 構造体に集約し、実行中に変化して `Changed<T>` で絞り込みたい「破れているか」だけを
  `BrokenEdges` という独立コンポーネントに切り出した。判別マーカーでもなく粒度も同じ不変データを
  フィールドごとに別コンポーネントへ割るのは過剰分割なので避けている（`DeathZone` を `Wall` から
  分けたような「振る舞いの判別マーカー」の分離とは目的が別）。
- **更新経路を一本化する**: `BrickDestroyed` イベント経由の一本道でしか `BrokenEdges` は
  変化しない設計にすることで、「かつてブロック同士が接していた面が、空洞との接触面に変わった
  時だけギザギザ化する」という要求を、個別の条件分岐の積み重ねではなく構造そのもので保証している。
- **依存クレートを増やさない自作 PRNG**: wasm32 ターゲットでの `getrandom` 系 feature-flag
  問題を避けつつ、見た目の再現性（同じブロック・同じ辺は毎回同じ形）を確保するため、
  xorshift32 ベースの `TearRng` を自作した（`tear.rs`）。
- **中点変位法の実行部を `tear.rs` に分離**: 乱数・シード・再帰と `TEAR_*` 定数の使用を
  `tear` モジュールに隠蔽し、`rendering.rs` は入口 `push_torn_edge` を 1 行呼ぶだけにした
  （Mesh 構築の主関心と、ギザギザ生成の実装詳細を分離）。

## 8. 実装の経緯（メモ）

Workflow ツールで「実装 → 独立レビュー → 修正+ビルド確認」の 3 段エージェント構成で実装した。
実装後、ユーザーから「ギザギザが分かりにくいので `TEAR_ROUGHNESS` / `TEAR_DEPTH` を調整したい」
というフィードバックがあり、各パラメータの意味（6 節の表）を説明した上で調整した。視認性重視で
段階的に強め、最終的に `TEAR_DEPTH = 2` / `TEAR_ROUGHNESS = 0.45`（＝より粗く、より大きく振れる
＝破れが目立つ）に落ち着いている（初期値は 0.18、途中 0.27 を経由）。star-shaped 制約は
各値で回帰テストが担保。

## 9. 関連ファイル

- `game_engine/src/components.rs` — `Brick`（`size` / `cell` / `fill` を集約したコンポーネント）、値型 `BrickCell` / `BrickFill`、独立コンポーネント `BrokenEdges`、イベント `BrickDestroyed`
- `game_engine/src/injection.rs` — `BrickLayout.cells` の算出（デフォルト / JS 注入）
- `game_engine/src/rendering.rs` — `spawn_brick` / `build_brick_mesh` / `redraw_broken_bricks`
- `game_engine/src/tear.rs` — `push_torn_edge`（入口）/ `midpoint_displace` / `TearRng` / `seed_for`（中点変位法の実行部）
- `game_engine/src/systems.rs` — `check_for_collisions` / `mark_broken_edges_on_brick_destroyed`
- `game_engine/src/config.rs` — `TEAR_DEPTH` / `TEAR_ROUGHNESS`
