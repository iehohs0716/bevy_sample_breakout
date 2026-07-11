# 背景画像の差し替え

Bevy アプリはこのフォルダの **`background.png`** を盤面の背景として読み込みます
（`game_engine/src/main.rs` の `BACKGROUND_IMAGE_PATH`）。

- 画像サイズ: 盤面は 900 × 600。同じ比率の画像だと綺麗に収まります（自動で引き伸ばされます）。
- 形式: PNG（`.png`）。別形式にする場合は `BACKGROUND_IMAGE_PATH` の拡張子も変更。

## 差し替え方法

### ブラウザ版（`pnpm dev` 中）
配信元は `frontend/public/assets/backgrounds/` です。ここの `background.png` を差し替えると、
Vite が検知して**自動でページがリロード**され背景が変わります。

```bash
# 例: 同梱サンプルに差し替え
cp frontend/public/assets/backgrounds/sample_sunset.png \
   frontend/public/assets/backgrounds/background.png
```

注意: `pnpm build:wasm` を再実行すると `game_engine/assets/` の内容で
`frontend/public/assets/` が上書きされます。恒久的に既定背景を変えたい場合は
この `game_engine/assets/backgrounds/background.png`（＝ソース側）を差し替えてください。

### ネイティブ版（`cargo run`）
この `game_engine/assets/backgrounds/background.png` を差し替えて起動し直すと反映されます。

## 同梱サンプル
- `background.png` … 既定（濃紺→紫のグラデーション）
- `sample_grid.png` … チェッカーボード
- `sample_sunset.png` … 夕焼け（オレンジ→ピンク）

## 将来: S3 など外部 URL から読む場合
`BACKGROUND_IMAGE_PATH` を外部 URL に変更し、Web では `bevy_web_asset` などの
HTTP アセットソースを追加する構成に切り替えます（別途対応）。
