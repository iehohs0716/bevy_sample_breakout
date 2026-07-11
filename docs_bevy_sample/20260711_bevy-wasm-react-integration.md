# Bevy Breakout を WASM 化して React 上で動かす構成

日付: 2026-07-11

`game_engine/`（Bevy 0.19 Breakout）を WASM ビルドし、別フォルダの React アプリ（`frontend/`）の
canvas に埋め込んでブラウザで動かすまでの構成メモ。ネイティブ実行（`cargo run`）も維持している。

---

## 全体像

```
Bevy(Rust) --cargo build--> .wasm --wasm-bindgen--> JS グルー + .wasm
   → frontend/public/wasm/ に配置
   → React が実行時 import して init() を呼ぶ
   → Window.canvas で指定した #bevy-canvas に描画
```

- フロント基盤: Vite + React + TypeScript（pnpm）
- 受け渡し: wasm-bindgen `--target web` の出力を `frontend/public/` へ

## ディレクトリ

- `game_engine/src/main.rs` — `WindowPlugin` で `canvas: Some("#bevy-canvas")` を指定
- `game_engine/.cargo/config.toml` — dev 用 wasm-server-runner ランナー
- `game_engine/assets/sounds/breakout_collision.ogg` — Bevy 公式 v0.19.0 から取得
- `frontend/scripts/build-wasm.sh` — WASM ビルド一括スクリプト
- `frontend/src/components/BevyGame.tsx` — canvas + WASM ロード
- `frontend/public/wasm/`, `frontend/public/assets/` — 生成物（gitignore）

## セットアップ（一度きり）

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126   # ← Cargo.lock の wasm-bindgen と一致必須
cd frontend && pnpm install
```

このリポジトリ環境では pnpm が素の PATH に無く mise 経由。実行は:
`mise exec pnpm@10.27.0 -- pnpm <cmd>`

## ビルド & 実行

```bash
cd frontend
pnpm build:wasm     # cargo build → wasm-bindgen → assets コピー
pnpm dev            # http://localhost:5173 (本記録では 5188 で検証)
```

ブラウザで開くと canvas に Breakout が表示され、← / → でパドル操作。

## 検証済み事項（2026-07-11）

- `pnpm build:wasm` 成功。`public/wasm/breakout.js`(113KB) + `breakout_bg.wasm`(約57MB) 生成
- dev サーバーで各リソースが 200 応答:
  - `/wasm/breakout.js` → text/javascript
  - `/wasm/breakout_bg.wasm` → application/wasm
  - `/assets/sounds/breakout_collision.ogg` → audio/ogg
- グルーの export は `export { initSync, __wbg_init as default };`。
  `init()` は引数省略時 `import.meta.url` 基準で `breakout_bg.wasm` を自動 fetch する。
- ブラウザ表示は当初「真っ白で何も出ない」状態から、下記の #4 → #2 の 2 バグを
  修正して canvas に描画されるようになった（← / → でパドル操作可能）。

---

## 実際にハマった順序（このセッションの流れ）

1. ビルド・配信までは一度で成功（wasm-bindgen 出力・アセット・dev サーバーの 200 を確認）。
2. ブラウザで開くと Vite が `Failed to load url /wasm/breakout.js ... This file is in /public`
   エラー（下記 #4）。→ 実行時に完全 URL を組み立てて import する方式で解消。
3. それでも画面が真っ白。原因は StrictMode + `cancelled` フラグで唯一の init が
   中断されていた（下記 #2）。→ 中断ロジックを削除して描画成功。

教訓: 「ビルド成功＝表示成功」ではない。WASM ロードの経路（バンドラのガード）と
React のライフサイクル（StrictMode 二重実行）の 2 段構えでつまずきやすい。

---

## 落とし穴・注意点

### 1. wasm-bindgen CLI と crate のバージョン一致
CLI（`cargo install wasm-bindgen-cli`）が `Cargo.lock` の `wasm-bindgen`（0.2.126）と
ズレるとロード時に schema mismatch エラー。`--version` で固定する。

### 2. React StrictMode の二重マウント（★真っ白の原因その2）
StrictMode は開発時に effect を「マウント→クリーンアップ→再マウント」と 2 回実行する。
Bevy(winit) は二重初期化・破棄ができないため `useRef` ガードで init を一度だけ呼ぶ。

**やってしまったバグ**: 併せて `let cancelled` フラグを置き、クリーンアップで
`cancelled = true`、`await import()` 後に `if (cancelled) return` としていた。すると:
1. 1回目 effect が起動 → クリーンアップで `cancelled=true`
2. 2回目 effect は ref ガードで即 return
3. 1回目の async が import から戻ると `cancelled` が true で **init を呼ばず終了**

→ 唯一の初期化が中断され画面が真っ白になった。Bevy はどのみち ref ガードで一度きりに
保証されるので、**クリーンアップで init を中断しない**（`cancelled` を廃止）のが正解。
SPA での再マウントも同様の既知制約（Bevy Discussion #12195）。

### 3. init() の "例外" は正常
winit は制御フローに例外を使うため、init() が
`"Using exceptions for control flow, don't mind me..."` を投げる。これは無視する。

### 4. Vite が `/public` の import を拒否する（★真っ白の原因その1）
`import("/wasm/breakout.js")` すると Vite が次のエラーを出す:
`Failed to load url /wasm/breakout.js ... This file is in /public and will be copied
as-is ... should not be imported from source code.`

Vite は `/public` 配下のファイルをソースからの import 対象にできない。`/*@vite-ignore*/`
+ パス変数化だけでは、`/` 始まりの絶対パスが Vite の解決対象として捕捉され回避できない。

**解決**: 実行時に**完全な絶対 URL** を組み立て、外部モジュールとして import する。
```ts
const wasmUrl = new URL("/wasm/breakout.js", window.location.origin).href;
const mod = await import(/* @vite-ignore */ wasmUrl);
```
完全 URL にすると Vite は外部扱いで解決をスキップし、ブラウザがネイティブに fetch する
（dev では `/public` 静的配信、本番では dist ルートへコピーされ、同じパスで動作）。
なお TS 側も絶対パス指定子はアンビエント型宣言でマッチできないため、この変数化で
静的モジュール解決を回避している。

### 5. アセットのパス
Bevy は Web 上でアセットをページルート相対 `/assets/...` から fetch する。
そのため `game_engine/assets/` を `frontend/public/assets/` にコピーする。

### 6. wasm サイズ（約57MB）
未最適化。配信サイズ削減には binaryen の `wasm-opt` が有効:
`brew install binaryen` すると `build-wasm.sh` が自動で `wasm-opt -Oz` を実行する。
`Cargo.toml` の `[profile.release]`（`opt-level="s"`, `lto=true`, `codegen-units=1`）も削減に寄与。

---

## 背景画像の差し替え機能（2026-07-11 追加）

> その後、背景画像を Rust ハードコードではなく **React 側から渡す**方式に発展させた。
> 設計と実装は [[20260711_react-to-bevy-background-injection]]、外部 URL の CORS / 画像
> フォーマット対応は [[20260711_external-image-cors-and-formats]]、実ブラウザ検証手法は
> [[20260711_wasm-bevy-browser-verification]] を参照。

### 要件
「Bevy アプリ自体がフォルダの画像ファイルを見て、差し替えたら背景が変わる」。
完全ライブ（リロード無し）は不要。将来 S3 等の外部パスに変わる可能性あり。

### 設計上の重要な制約
ブラウザ上の WASM は**サンドボックスにより任意のローカルファイルを直接読めない**。
読めるのは Web サーバーが配信するファイル（＝`frontend/public/assets/`）のみ。
よって「ローカルフォルダ」= 配信対象の `public/assets/backgrounds/` を指す。

### 実装
- Bevy(`main.rs`): `BACKGROUND_IMAGE_PATH = "backgrounds/background.png"` を
  盤面全体（900×600）を覆うスプライトとして `z=-10`（最背面）に spawn。
  画像未配置時は `ClearColor`（薄グレー）がフォールバックとして見える。
- 画像パスは**定数1箇所**。将来 S3 URL 等へ切り替える場合はここを変更
  （Web で外部 URL を読むには `bevy_web_asset` 等の HTTP アセットソースが別途必要）。
- 同梱サンプル: `background.png`（既定）/ `sample_grid.png` / `sample_sunset.png`
  （`game_engine/assets/backgrounds/`。純正 Python で生成した 900×600 PNG）。
- Vite プラグイン `bevyDevServer`(`vite.config.ts`): `public/assets/backgrounds/`
  を watch し、差し替え時に `full-reload` を送る。public 配下はモジュールグラフに
  乗らず HMR が効かないため、watcher を明示している。同プラグインで後述の `.meta` 404 も担当。

### 差し替え運用
- ブラウザ dev: `frontend/public/assets/backgrounds/background.png` を差し替え → 自動リロード。
  ただし `pnpm build:wasm` 再実行で `game_engine/assets/` の内容に上書きされる点に注意。
- 既定背景を恒久変更: ソース側 `game_engine/assets/backgrounds/background.png` を差し替え。
- ネイティブ: `game_engine/assets/backgrounds/background.png` を差し替えて再起動。
- 詳細は `game_engine/assets/backgrounds/README.md` を参照。

### 実装後に判明した重要バグ（Playwright で実ブラウザ検証して発見）

最初「背景が表示されない／画面が縮む」不具合が出た。Playwright(chromium + SwiftShader)で
実際にページを開き、コンソール・ネットワーク・スクリーンショットを取得して原因特定した。

#### バグ A: `.meta` フォールバックでアセットが読めない（★背景が出ない主因）
Bevy はアセット取得時に `<asset>.meta`（例 `background.png.meta`）も fetch する。
存在しないと Vite の **SPA フォールバックが `index.html`(200) を返す** ため、Bevy が
その HTML を RON メタとして解析失敗し、**アセット本体のロードが中断**する。
コンソール: `Failed to deserialize meta for asset ... ExpectedNamedStructLike("AssetMetaMinimal")`。
→ **解決**: `vite.config.ts` の `bevyDevServer` で `*.meta` へのリクエストに 404 を返す
ミドルウェアを追加。404 なら Bevy は「メタ無し（既定）」として本体を正しく読む。
（画像だけでなく音声 `.ogg.meta` も同じ問題だった）

#### バグ B: `fit_canvas_to_parent` で canvas が縮小・崩壊
`WindowPlugin` の `fit_canvas_to_parent: true` は canvas を親要素サイズに合わせる。
親（`.game-frame`）のサイズが未定義だと canvas と相互参照して縮小（崩壊）する。
→ **解決**: `App.css` で `.game-frame` を固定 900×600 にし、`canvas { width/height:100% }`。

#### 対策 C: wasm のキャッシュバスター
57MB の `breakout_bg.wasm` はファイル名固定でブラウザが旧ビルドをキャッシュしやすい。
`BevyGame.tsx` で `init({ module_or_path: "/wasm/breakout_bg.wasm?t=<Date.now()>" })` と
明示し、リロード毎に最新を読ませる（開発時のキャッシュ事故防止）。

#### 検証手段メモ
Playwright MCP は本環境に未接続だったため、`playwright` を直接スクリプト実行して検証した
（chromium を `--enable-unsafe-swiftshader --use-gl=angle --use-angle=swiftshader` で起動すると
ヘッドレスでも WebGL2 で Bevy が描画でき、スクリーンショットで背景描画を目視確認できる）。
