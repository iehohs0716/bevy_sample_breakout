# 画像差分による自動ブロック配置（brick diff auto layout）

日付: 2026-08-02

**実装済み。** 以下は当初の設計メモをベースに、実装時に確定した内容へ更新したもの
（差分の詳細は 9 節）。背景画像・ブロック画像の注入方式は
[[20260711_react-to-bevy-init-params]] / [[20260711_react-to-bevy-background-injection]] /
[[20260715_brick-image-rendering]]、比率維持の内接表示は [[20260715_aspect-ratio-and-letterbox]]
を前提とする。

## 1. 要件

現状、ブロック配置は次の 2 択しかない（`game_engine/src/injection.rs`）。

- `injected_brick_layout()` — JS が `bricks: [{x,y}, ...]` を明示指定
- `default_brick_layout()` — 指定が無ければアリーナ全体を `BRICK_SIZE` で敷き詰めるフォールバック

今回追加したいのは 3 つ目の経路: **JS が「背景画像」と「ブロック画像」の 2 種類を注入し、
かつ `bricks`（明示配置）を渡していない場合、この 2 画像を見比べて「絵柄が違う場所」だけを
自動でブロックにする。** 同じに見える場所（＝ブロックを置いても置かなくても見た目が変わらない
場所）にはブロックを生成しない。

加えて、生成範囲を **バー（パドル）からアリーナ天井までの高さの 20% 以上** に制限する
（＝パドルに近い下側 20% には自動生成しない）。この閾値は `game_engine/src/config.rs` の
定数として調整可能にする。

## 2. 発火条件（フォールバックの優先順位）

`systems/setup.rs` での解決順序を次のように変更する。

1. `brick_layout_override.0.take()` が `Some` → **最優先。従来どおり**そのまま使う（今回の
   自動生成は一切関与しない）。
2. 上記が `None` かつ、背景画像・ブロック画像の **両方が JS 注入（Override）由来** →
   新設する `diff_brick_layout(...)` で自動生成する。
3. 上記のどちらでもない（画像が 0 枚 or 1 枚だけ、あるいはブロック位置もどちらの画像も無い）
   → 従来どおり `default_brick_layout(paddle_y)`（アリーナ全体敷き詰め）。

「両方が Override 由来」に限定する理由: デフォルト背景（`BACKGROUND_IMAGE_PATH` を
`asset_server.load` する経路）は非同期ロードで、`setup` の時点ではまだピクセルデータが
揃っている保証がない。差分計算に使える生ピクセルを同期的に持っているのは、JS から
バイト列で渡されデコード済みの `BackgroundOverride` / `BrickImageOverride` の場合だけ。

## 3. アルゴリズム

### 3.1 候補グリッド

`default_brick_layout` と同じグリッド分割ロジック（`BRICK_SIZE` / `GAP_BETWEEN_BRICKS` /
`GAP_BETWEEN_BRICKS_AND_SIDES` / `GAP_BETWEEN_BRICKS_AND_CEILING` / `GAP_BETWEEN_PADDLE_AND_BRICKS`
を使った列数・行数・オフセット計算）をそのまま流用し、`(row, col)` ごとの中心座標
`Vec2` の全候補を作る。ロジックを複製せず、`default_brick_layout` 内の列/行/オフセット計算部分を
共有できる形に切り出す（詳細は 4 節）。

`cells` の `row`/`col` は候補グリッド上の **絶対インデックス** をそのまま使う（間引いた後も
振り直さない）。理由: `BrokenEdges` の隣接破壊面判定（`systems::update::brick`）は
`(row, col)` の隣接一致で「隣にブロックがあるか」を見ているはずで、絶対インデックスを保てば
「差分で間引かれて隣が無い」状態を自然に表現できる（振り直すと本来隣接していたセルが
非隣接番号になり、逆に本来非隣接だったセルが隣接番号になりうる）。

### 3.2 高さ制限

候補セルの中心 y について

```
height_ratio = (cell_center.y - paddle_y) / (TOP_WALL - paddle_y)
```

を求め、`height_ratio < BRICK_DIFF_LAYOUT_MIN_HEIGHT_RATIO`（新設 config 定数、既定 0.2）の
セルは候補から除外する。除外は「差分判定より前」に行う＝差分があっても対象外なら生成しない。

### 3.3 差分判定

残った候補セルごとに、**背景画像・ブロック画像それぞれについて独立に**「アリーナに
contain フィットで貼ったと仮定したときの、このセルが覆う領域」のピクセル矩形を求める
（`common/brick/texture_crop.rs` の `brick_image_rect` と全く同じ写像。両画像とも同じ
アリーナ矩形 `Vec2::new(RIGHT_WALL-LEFT_WALL, TOP_WALL-BOTTOM_WALL)` に内接させているため、
同じワールド座標のセルが両画像それぞれの独立した内接矩形へ正しく写像される）。

各矩形内の平均色（RGB）を求め、2 色のユークリッド距離が `BRICK_DIFF_COLOR_THRESHOLD`
（新設 config 定数）を超えたセルだけを「差分あり」として採用する。

- どちらかの画像でセルが内接矩形の外（レターボックス部分）に出る場合は、その画像側のサンプルを
  「黒」とみなして比較する（`brick_image_rect` が `None` を返す場合の既存の黒塗りフォールバックと
  一貫させる）。
- 平均色の算出には Bevy `Image::get_color_at(x, y) -> Result<Color, TextureAccessError>`
  （0.19 で利用可能。RGBA 8bit/16bit/32bit・float 系フォーマットに対応）を使い、
  `Color::to_srgba()` で得た 0..1 の RGBA を平均する。
- しきい値 `BRICK_DIFF_COLOR_THRESHOLD` は 0..1 スケール（`to_srgba()` の値域）での
  RGB のユークリッド距離。初期値は `0.1`（実画像で試して調整する前提の仮値）。

### 3.4 BrickLayout 構築

差分ありと判定されたセルの `positions` / `cells` を集めて
`BrickLayout { positions, cell_size: BRICK_SIZE, cells }` を返す。空（1 つも差分が
無かった）の場合は `None` を返し、呼び出し側で `default_brick_layout` にさらにフォールバックする。

## 4. モジュール配置方針

CLAUDE.md の `util` / `common` / `injection` の使い分けに沿う。

- **`util`**: ドメイン型（`Brick` 等）に依存しない純粋な幾何計算をここに集約する。
  - 既存の `common/brick/texture_crop.rs::brick_image_rect` は「`position, size` のセルを、
    `container`（アリーナ）に contain フィットで貼った `image_size` の中の、どのピクセル矩形に
    対応するか」という**ドメイン非依存の計算**なので、`util::inscribed_source_rect(position, size,
    container, image_size) -> Option<Rect>` のような汎用関数として `util` に引き上げ、
    `texture_crop.rs` 側はそれを呼ぶ薄いラッパーにする（アリーナ矩形を固定引数として渡すだけ）。
    これにより「セル座標→画像内ピクセル矩形」の写像ロジックが二重実装にならず、UV 切り出しと
    差分サンプリングの両方から共有できる。
  - 矩形内の平均色を求める関数（例: `util::average_color(image: &Image, rect: Rect) -> Vec4`）も
    ここに置く。`bevy::Image` はエンジン型であって本プロジェクト固有のドメイン型ではないため、
    ここでの使用は `util` の「ドメイン型に依存しない」方針に反しない。
- **`injection.rs`**: 「候補グリッド生成 → 高さ制限 → 差分判定 → `BrickLayout` 組み立て」という
  一連の処理は `BrickCell` / `BrickLayout` という**ブロックドメインの型を組み立てる処理**であり、
  かつ `default_brick_layout` と同じく「JS 注入データ（または不在）から初期 `BrickLayout` を
  導出する」役割なので、既存の `default_brick_layout` / `injected_brick_layout` と並べて
  `injection.rs` に `diff_brick_layout(background: &Image, brick_image: &Image, paddle_y: f32)
  -> Option<BrickLayout>` として追加する。`Query` を取らない普通の関数であり、`setup` から
  手動で呼ばれる点も既存 2 関数と同じ。
  - 追加後にファイルが大きくなりすぎるようなら、`default_brick_layout` を含めて
    `injection/brick_layout.rs` のようなサブモジュールへ切り出すことを検討する
    （`common/brick/{mesh,texture_crop,torn_edge}` と同じ分割パターン）。今回はまず
    `injection.rs` 直下に追加し、行数を見て判断する。
- **`config.rs`**: 新設定数 2 つ（4.1 節）を追加する。

## 5. config.rs への追加

```rust
/// 画像差分による自動ブロック生成を許可する高さの下限比率。
/// 0.0 = バー（パドル）の位置、1.0 = アリーナ天井（`TOP_WALL`）。
/// この比率未満の高さには（画像差分があっても）ブロックを生成しない。
pub const BRICK_DIFF_LAYOUT_MIN_HEIGHT_RATIO: f32 = 0.8;

/// 背景画像とブロック画像の同一セル領域の平均色を比較し、ブロックを生成するかどうかを
/// 判定するしきい値（RGB のユークリッド距離。各チャンネル 0..1 スケールなので最大 `sqrt(3)` 程度）。
/// これを超えたセルだけを「差分あり」としてブロックにする。実画像で試して調整する前提の仮値。
pub const BRICK_DIFF_COLOR_THRESHOLD: f32 = 0.1;
```

## 6. setup.rs への組み込み方針

現状の該当行:

```rust
let brick_layout = brick_layout_override
    .0
    .take()
    .unwrap_or_else(|| default_brick_layout(paddle_y));
```

これを 3 段フォールバックに変更する。ポイントは、差分計算に使う生ピクセルを
`Assets<Image>` 登録**後**の `Handle` から取り直せること（`RenderAssetUsages::default()` で
CPU 側データも保持しているため `images.get(&handle)` で参照できる）。よって「画像を
`Assets<Image>` に登録するタイミング」自体は今のまま変更不要で、登録済みの `Handle` から
`images.get()` して差分計算に使えばよい。

ただし「背景が Override 由来かどうか」は `background_override.0.take()` の `match` の
どちらの枝を通ったかで一度失われるので、`bool` で保持しておくか、`take()` 前に
`is_some()` を見ておく必要がある（発火条件は「両方 Override 由来」なので、
デフォルトアセット経由の背景では diff を発火させない）。

## 7. 未決事項・要検証（実装時に判断する）

- `BRICK_DIFF_COLOR_THRESHOLD` の具体的な妥当値は実画像で試すまで分からない。まず仮値で実装し、
  Playwright での見た目確認をしながら調整する。
- ブロック画像が透過 PNG の場合、アルファチャンネルを平均色比較にどう組み込むか
  （今回の方針では単純化のため RGB のみ比較し、透過対応は必要になった時点で拡張する）。
- 差分セルが飛び飛び（連続しない）になった場合の見た目は画像デザイン依存として許容する
  （アルゴリズム側で穴埋め・平滑化はしない）。
- `Image::get_color_at` 系 API が Bevy 0.19 でどう呼べるか（rust-analyzer チェック時に確認。
  無ければ生バイトから手計算するフォールバックに切り替える）。

## 8. 関連ファイル（実装箇所）

- `game_engine/src/config.rs` — 新設定数 2 つ
- `game_engine/src/util.rs` — `inscribed_source_rect` / `average_color` の追加
- `game_engine/src/common/brick/texture_crop.rs` — `brick_image_rect` を `util` 呼び出しの
  薄いラッパーに置き換え
- `game_engine/src/injection.rs` — `brick_grid_candidates`（`default_brick_layout` からの
  グリッド計算切り出し）と `diff_brick_layout` を追加
- `game_engine/src/systems/setup.rs` — 3 段フォールバックへの変更
- `game_engine/assets/backgrounds/diff_background.png` / `diff_brick_image.png` — 動作確認用の
  デモ画像（9 節）
- `frontend/src/entities/level/api/mockLevels.ts` — デモ画像を使うレベル
  `diff-auto-layout-sample` を追加

`injection.rs` は追加後も見通せる行数だったため、当初検討していたサブモジュール分割
（`injection/brick_layout.rs`）は行わず、そのまま直下に置いた。

## 9. 実装時に確定した詳細・注意点

- **`Handle<Image>` の使い回し**: `setup.rs` は元々 `background_handle` を背景 `Sprite` の
  `image` フィールドへ move していたため、そのままでは差分判定で `images.get(&background_handle)`
  できない（move 済みで参照不可）。`Sprite` 側は `background_handle.clone()`（`Handle` は
  Arc ベースで clone は軽量）を渡すよう変更し、元の `background_handle` を差分判定用に残した。
- **発火条件の実装**: `background_override.0.take()` で `Some`/`None` を判定する前に
  `background_was_overridden = background_override.0.is_some()` を退避しておき、
  ブロック配置解決時にこの `bool` で「両方 Override 由来か」を判定する
  （`bool::then(...).flatten()` で `Option<BrickLayout>` を得て `unwrap_or_else` で
  `default_brick_layout` にフォールバック）。
- **差分の比較対象**: 平均色は `to_srgba()` の RGBA（0..1）から RGB のみ（`Vec4::truncate()`）
  を取り出してユークリッド距離を取る。アルファは未対応（7 節の未決事項のまま）。

## 10. 動作確認（デモ）

`frontend/src/entities/level/api/mockLevels.ts` に `diff-auto-layout-sample` を追加した
（レベル一覧画面から「自動配置サンプル（画像差分）」として遷移できる）。

- `diff_background.png`: 空色→濃紺の縦グラデーション（900x600、アリーナと同寸法で
  レターボックス無し）。
- `diff_brick_image.png`: 背景と同じ絵に、次の 2 箇所だけ絵柄を変えて重ねたもの。
  - 上端の帯（高さ約 110px ≒ 画像上端からアリーナ天井側）: 5 色の帯。
  - 中段の帯（マゼンタ、高さ約 100px）: 意図的に絵柄を変えた領域。
    `BRICK_DIFF_LAYOUT_MIN_HEIGHT_RATIO` の値次第で、ここが高さ制限の対象外
    （＝差分があってもブロック化されない）になるかどうかが変わる。**この定数の値は
    `config.rs` を参照**（動作確認時の値によって、この帯が全く見えない／一部だけ
    ブロック化される、のどちらの見た目にもなり得る。高さ制限そのものの効き方を
    確認したい場合は、この帯とアリーナ天井までの距離・比率を計算して閾値と比較する）。
- `bricks: []`（明示配置なし）とし、`background` / `brickImage` の両方を指定することで
  発火条件（2 節）を満たす。

Playwright で `/play/diff-auto-layout-sample` を確認し、上端の帯が実際にブロックとして
描画されることを確認した（背景と絵柄が異なる領域だけが差分ありと判定されている）。
中段のマゼンタ帯がブロック化されるかどうかは前述の通り閾値依存で、閾値を緩めると
（高さ制限の対象範囲が広がると）マゼンタ側も部分的にブロック化される。マゼンタ帯が
高さ制限で除外されている場合は、その領域にブロックが spawn されないため
`brick_image` 側の「窓」が開かず、背景がそのまま見える
（[[20260715_brick-image-rendering]] の「壊すと背景が覗く」と同じ仕組みで、
最初から窓が無い状態）。`window.__BREAKOUT_CONFIG__` も `backgroundBytes` /
`brickImage` が共に設定され `bricks` が未設定であることを確認済み。
既存の明示配置レベル（`grid-block` 等）にも見た目の回帰が無いことを確認した。

## 11. 追加対応: `cellSize` を `bricks` から独立させる

実装後、「`bricks`（明示配置）を渡さないが `cellSize` は指定したい」ケース
（デフォルト敷き詰め・画像差分自動配置のどちらでも、格子の粗さだけ変えたい）が
考慮できていないことが判明した。当初の実装は次の 2 段構えで `cellSize` を握りつぶしていた。

- **フロント（`startBevyGame.ts`）**: `cellSize` は `bricks.length > 0` の分岐の中でしか
  `window.__BREAKOUT_CONFIG__` に載せていなかった。`bricks` が空だと `cellSize` を渡しても
  黙って捨てられる。
- **Rust（`injection.rs`）**: `default_brick_layout` / `diff_brick_layout`（と共有ロジックの
  `brick_grid_candidates`）はいずれもグリッド計算に `config::BRICK_SIZE` を直接埋め込んで
  いた。`cellSize` が読まれるのは `injected_brick_layout()`（＝`bricks` 配列がある経路）だけ。

### 対応内容

- `startBevyGame.ts`: `cellSize` を `bricks` の分岐から外に出し、指定があれば常に
  `config.cellSize` に載せるようにした。
- `injection.rs`:
  - `cellSize` の読み取りロジックを `read_cell_size(config: &JsValue) -> Option<Vec2>` として
    切り出し、`injected_brick_layout()`（`bricks` とセットの場合）と、新設した
    `injected_cell_size() -> Option<Vec2>`（`bricks` の有無に関わらず読む。ネイティブビルドは
    常に `None`）の両方から使う。
  - `brick_grid_candidates` / `default_brick_layout` / `diff_brick_layout` はいずれも
    `cell_size: Vec2` を引数に取るよう変更し、内部の `BRICK_SIZE` 直書きを置き換えた。
    値の決定（`injected_cell_size().unwrap_or(BRICK_SIZE)`）は呼び出し側（`setup.rs`）が行う
    （関数自体は入力を渡された通りに使うだけの「純粋」な形を保つ）。
- `setup.rs`: `cell_size` を一度だけ解決し、ブロック配置解決の 3 段フォールバック
  （明示配置 → 差分自動配置 → デフォルト敷き詰め）のうち後者 2 つに共通で渡す。
- `widgets/bevy-game/model/types.ts`: `cellSize` の JSDoc を「`bricks` の有無に関わらず効く」
  内容に更新（旧: 「`bricks` とセットの時のみ効く」）。

### 動作確認

`diff-auto-layout-sample` の `cellSize` を `{ width: 50, height: 30 }`（デフォルトと同値、
差が見えない）から `{ width: 25, height: 20 }` に変更して Playwright で再確認。マゼンタ帯が
ブロック化される領域が、旧セルサイズ（30px 高）より明らかに薄い帯（20px 高相当）として
描画され、`bricks` 無しでも `cellSize` が効いていることを視覚的に確認した。
