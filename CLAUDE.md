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

コードを変更したら「ビルドが通った」で終わらせない。**必ず WASM 再ビルド → 実ブラウザ
（Playwright）で描画確認**まで行うこと。canvas 描画系の WASM は「ビルド成功／配信 200」
だけでは正常性を保証できない。

### 1. WASM 再ビルド

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

### 2. Playwright MCP で描画確認

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
