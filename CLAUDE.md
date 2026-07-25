# CLAUDE.md

Bevy 製ブロック崩し（`game_engine`）を WASM ビルドし、React フロント（`frontend`）から
起動・制御するサンプル。以下の規約に従うこと。

## プロジェクト構成

- `game_engine/` — Bevy ゲーム本体（Rust）。`src/` は次のモジュールに分割。
  - `config`: ゲーム全体の定数
  - `components`: Component / Resource / Event の定義
  - `injection`: React(JS) から渡される初期化パラメータの読み取り
  - `notify`: ゲームイベント（クリア等）をフロント(JS)へ通知
  - `rendering`: 画像フィット計算とブロック描画ヘルパー
  - `setup`: 起動時セットアップ system（`Startup` に登録）
  - `systems`: 毎フレームのゲームプレイ system
- `frontend/` — Vite + React。WASM グルーの読み込みと初期化パラメータ受け渡しを担う。

## ビルド規約（必ずこの流れを踏む）

コードを変更したら「ビルドが通った」で終わらせない。**必ず rust-analyzer によるチェック →
WASM 再ビルド → 実ブラウザ（Playwright）で描画確認**まで行うこと。canvas 描画系の WASM は
「ビルド成功／配信 200」だけでは正常性を保証できない。

### 1. rust-analyzer によるチェック（必須）

Rust コードを変更したら、**まず rust-analyzer で診断を確認する**こと。型・借用・
未使用インポート等の問題を、WASM ビルドを待たずに最速で潰すための必須ステップ。

- エディタ（VS Code の rust-analyzer 拡張等）の診断でエラー／警告が 0 であることを確認する。
- CLI で確認する場合は `cargo check --target wasm32-unknown-unknown`（rust-analyzer と
  同じ `cargo check` ベースの診断）を通すこと。
- 注意: rust-analyzer は一部 **誤検知（false positive）** を出す。既知のものは対処法が
  確立している（例: `Single` の Deref に対する E0614 は `into_inner()` で回避、`Deref`
  派生 Resource への代入は `.0` を使う）。`rustc` / `cargo check` が通るのに rust-analyzer
  だけが赤線を出す場合は誤検知を疑い、既知パターンに沿って回避する。

### 2. WASM 再ビルド

```
cd frontend && pnpm build:wasm
```

内部で `frontend/scripts/build-wasm.sh` が動く:

1. `cargo build --release --target wasm32-unknown-unknown`
2. `wasm-bindgen --target web`（`breakout.js` + `breakout_bg.wasm` を生成）
3. `wasm-opt` があればサイズ最適化（`brew install binaryen` で有効化）
4. `game_engine/assets` を `frontend/public/assets` へコピー

注意:

- `public/wasm` / `public/assets` は gitignore 対象。チェックアウトごとに再ビルドが必要。
- `wasm-bindgen-cli` のバージョンは `Cargo.lock` と一致必須。
- `Cargo.toml` の feature 変更（例: `jpeg`/`webp`）を反映するにも再ビルドが必要。

### 3. Playwright MCP で描画確認

**描画確認は必ず Playwright MCP を使う。** 生の `playwright` を pip / npm で直接入れて
スクリプト実行するのは禁止（Python 環境などを汚すため）。

- 大きな wasm のロード + Bevy 起動 + 描画に時間がかかる。ページ読み込み後に
  **十分（目安 20 秒）待ってから** `#bevy-canvas` のスクリーンショットを取る。
- スクリーンショット + コンソールログ + `window` グローバル（例
  `window.__BREAKOUT_CONFIG__`）の三点で確証を取る。
- 無視してよいノイズ: `favicon.ico` の 404 / AudioContext autoplay 警告 /
  SwiftShader 由来の software rendering WARN / winit の "Using exceptions for control flow"。

## エンティティ・コンポーネント設計方針

**役割（セマンティクス）が違うものは、同じエンティティ／コンポーネントで使い回さない。**
1 つの型にフラグや分岐を足して複数の意味を兼ねさせるのではなく、意味ごとに別の型として
分ける。振る舞いの差はマーカーコンポーネントの有無で表現し、`Query` で判別する。

例: アリーナ下端は反射する `Wall` ではなく、専用の `DeathZone` として分離している。

- `Wall`（`components.rs`）: 反射する壁。`WallLocation` は Left / Right / Top のみ。
- `DeathZone`（`components.rs`）: ボールが触れるとライフを減らす下端領域。見た目を持たず
  `Collider` のみ。`Wall` とは別コンポーネントなので `check_for_collisions` 側で
  `Option<&DeathZone>` により反射対象と区別できる。

新しい役割を追加するときは、既存の型に列挙子やフラグを足して兼用させず、この `DeathZone`
と同じように独立した型を切ること。

### ただし「分ける」の対象を取り違えないこと

上の「別の型に分ける」は、**振る舞いを判別するマーカー**（有無を `Option<&T>` / `With<T>`
で見て処理を分岐させるもの。例: `Brick` / `DeathZone`）に対する指針である。**同じ1エンティティに
常に同居し、粒度（カーディナリティ）が同じで、有無の判別にも使われないデータ**まで、フィールド
ごとに別コンポーネントへ割るという意味ではない。それは過剰分割で、クエリに冗長な `Option` が
並ぶだけになる。

同居する不変データは 1 つの型にまとめてよい。**分ける軸は「役割」ではなく「可変性・変更検知」**:

- **spawn 時に確定して以後変わらないデータ**はまとめる。
- **実行中に変化し、`Changed<T>` で対象だけを絞って処理したいものだけ**を独立コンポーネントに切る。

例: ブロックは、不変の `size` / `cell`（格子座標）/ `fill`（塗り方）を `Brick` に集約し、実行中に
変化する `BrokenEdges`（破れた辺）だけを別コンポーネントにして `Changed<BrokenEdges>` による
絞り込み再描画（`redraw_broken_bricks`）を効かせている。「これはブロックだ」という判別は
`With<Brick>` / `Option<&Brick>` で行い、破壊・加点・クリア判定・リセットに使う。
