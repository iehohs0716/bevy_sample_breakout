# 外部URL背景の CORS の壁と画像フォーマット(JPEG/WebP)対応

日付: 2026-07-11

[[20260711_react-to-bevy-background-injection]] のバイト列注入方式で、外部 URL の画像を
背景に使おうとして踏んだ 2 つのハマりどころ（CORS と画像フォーマット）と、その切り分け・対処の記録。

---

## ハマりどころ 1: 外部URLが CORS で fetch できない

### 症状

`background` に第三者サイトの画像 URL を指定したが、背景が変わらずデフォルトのままだった。
試した URL:

- `https://www.ktr.mlit.go.jp/ktr_content/content/000093427.jpg`
- `https://images.ygoprodeck.com/images/cards_cropped/6983839.jpg`

### 原因

どちらのサーバも他オリジン向けの `Access-Control-Allow-Origin`(ACAO) ヘッダを返さない。
ブラウザの `fetch()` が `TypeError: Failed to fetch`（`net::ERR_FAILED`）でブロックされ、
`window.__BREAKOUT_CONFIG__` がセットされないまま、Bevy 側がデフォルト背景にフォールバックしていた
（コンソールに `背景画像の取得に失敗しました…` の warn が出る）。

**注意**: `curl` で確認すると 200 が返るため「取れている」と誤解しやすい。curl は CORS を
強制しないので取得できてしまう。ブラウザだけがブロックする。

```bash
# 200 は返るが Access-Control-Allow-Origin ヘッダが無い＝ブラウザはブロックする
curl -sI -H "Origin: http://localhost:5173" \
  https://images.ygoprodeck.com/images/cards_cropped/6983839.jpg
```

### 切り分けの実演

同じ画像を**サーバ側(curl)で取得して同一オリジン(`frontend/public/assets/`)に置く**と、
背景としてちゃんと描画された。これにより:

- 画像自体・デコード・描画パイプラインは正常。
- 問題は「第三者サイトの画像をブラウザから直接 fetch できない」という Web の CORS 制約だけ。

と確定できた。実装のフォールバックも正しく機能し、CORS 失敗でもクラッシュせず
デフォルト背景で起動を継続した。

### 対処（3 案）

1. **画像を自分の配信元に置く**（`frontend/public/assets/` や自社ストレージ）。最も簡単。
2. **CORS 許可済みホストを使う**。例: S3 バケットに CORS 設定を入れる
   （`AllowedOrigins` に自オリジン、`AllowedMethods: ["GET"]`）。
3. **同一オリジンプロキシを立てる**。第三者 URL をそのまま使いたい場合、
   dev は vite の `server.proxy`、本番はリバースプロキシを用意し、
   React は `/proxy?url=...` のような同一オリジンパスを fetch する。

## ハマりどころ 2: JPEG/WebP はデフォルトでデコードできない

### 症状

同一オリジンに置いた JPEG 画像でも、取得はできているのに背景がデコードできず
表示されないケースがあった。

### 原因

bevy 0.19 の default features には `png` は含まれるが、`jpeg`/`webp`/`bmp` は**含まれない**
（`bevy-0.19.0/Cargo.toml` を確認）。`jpeg`・`webp` は正式な feature 名で、
それぞれ `bevy_internal/jpeg`・`bevy_internal/webp` → `image/jpeg`・`image/webp` を有効化する。
そのため JPEG のバイト列を渡しても `Image::from_buffer` がデコードに失敗していた。

### 対処

`game_engine/Cargo.toml` で `bevy` に feature を追加する（default features は維持したまま）:

```toml
# 背景画像は png に加えて jpeg / webp もデコードできるようにする（default features は維持）。
bevy = { version = "0.19.0", features = ["jpeg", "webp"] }
```

反映には **WASM 再ビルド**（`pnpm build:wasm`）が必要。

### 補足: MIME をそのまま渡せる

`Image::from_buffer` の `image_type` は `ImageType::MimeType("image/jpeg")` のように
MIME でも指定できる。React が `content-type` から取った値
（`backgroundMime`）をそのまま渡せるので、拡張子判定を Rust 側で書く必要はない
（`game_engine/src/main.rs:96` の分岐）。

## 検証で確認できたこと

- `sample_sunset.png`（PNG）: 背景として描画された。
- 同一オリジン配信の JPEG（`sips` で png→jpg 変換したもの、
  および ygoprodeck カード画像を curl で取得したもの）: `jpeg` feature 追加後に描画された。
- 外部 URL の直接指定: CORS でブロックされ、デフォルト背景にフォールバックした。

実ブラウザでの検証手順は [[20260711_wasm-bevy-browser-verification]] を参照。
