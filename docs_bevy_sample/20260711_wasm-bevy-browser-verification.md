# WASM/Bevy canvas を実ブラウザで検証する手法(Playwright + swiftshader)

日付: 2026-07-11

「ビルド成功で終わらせず、実ブラウザで描画確認する」方針での検証手順メモ。
背景差し替え機能（[[20260711_react-to-bevy-background-injection]]）や CORS 切り分け
（[[20260711_external-image-cors-and-formats]]）は、この手法で実際に描画を目視確認した。

---

## 前提: WASM の再ビルド

WASM 再ビルドは `frontend/scripts/build-wasm.sh`（`pnpm build:wasm`）で行う。処理内容:

1. `cargo build --release --target wasm32-unknown-unknown`
2. `wasm-bindgen --target web`（JS グルー `breakout.js` + `breakout_bg.wasm` を生成）
3. `wasm-opt` があればサイズ最適化（`brew install binaryen` で有効化）
4. `game_engine/assets` を `frontend/public/assets` へコピー

注意点:

- 出力 `breakout_bg.wasm` は約 57MB（未最適化時）。
- `public/wasm`・`public/assets` は **gitignore 対象**。チェックアウトごとに
  `pnpm build:wasm` が必要。
- `wasm-bindgen-cli` のバージョンは `Cargo.lock`（0.2.126）と一致必須。
- Cargo.toml の feature 変更（例: `jpeg`/`webp` 追加）を反映するにも再ビルドが必要。

## Playwright(headless chromium)で検証

WebGL2 を headless で動かすため、chromium の launch args に以下を付ける:

```
--use-gl=angle
--use-angle=swiftshader
--enable-unsafe-swiftshader
--ignore-gpu-blocklist
```

これで GPU が無いヘッドレス環境でも WebGL2 で Bevy が描画できる。
`AdapterInfo` が SwiftShader/CPU になり "software rendering" 警告が出るが、描画自体は可能。

## 待ち時間とスクリーンショット

57MB の wasm のロード + Bevy 起動 + 描画に時間がかかるため、
`wait_for_load_state("networkidle")` の後に **約 20 秒**待ってから
`#bevy-canvas` を `screenshot` する。

## React が背景を渡せているかの確証

`page.evaluate` で `window.__BREAKOUT_CONFIG__` を読み、
`backgroundBytes.length` と `backgroundMime` を確認する。これにより
「React が実際にバイト列を渡せているか」を確定できる。あわせてコンソールに
背景取得失敗/デコード失敗の warn が無いことも確認する。

## 無視してよいノイズ

- `favicon.ico` の 404
- AudioContext の autoplay 警告
- SwiftShader 由来の各種 WARN（software rendering など）
- winit の "Using exceptions for control flow, don't mind me..."（制御フロー用の例外）

## 実際に確認できたこと（2026-07-11）

- `sample_sunset.png`（PNG）: 背景として描画。
- 同一オリジン配信の JPEG（`sips` で png→jpg 変換したもの、
  ygoprodeck カード画像を curl 取得したもの）: `jpeg` feature 追加後に描画。
- 外部 URL の直接指定: CORS でフォールバックし、デフォルト背景で起動継続。

## 補足

Playwright MCP は本環境に未接続だったため、`playwright` を直接スクリプト実行して検証した。
canvas 描画系の WASM は「ビルド/配信の 200 応答」だけでは正常性を保証できないため、
スクリーンショット + コンソール + `window` グローバルの三点で確証を取るのが有効。
関連する過去の描画バグ（`.meta` 404 フォールバック、`fit_canvas_to_parent` 崩壊など）は
[[20260711_bevy-wasm-react-integration]] を参照。
