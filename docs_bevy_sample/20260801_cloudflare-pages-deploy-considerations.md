# Cloudflare Pages への frontend デプロイ検討（SPAルーティング／Rustビルド／WASM配信）

日付: 2026-08-01

`frontend`（Vite + React、Bevy WASM ゲームを canvas に埋め込む）を Cloudflare Pages に
デプロイしてよいかを調査した記録。関連: [[20260731_cloudflare-pages-functions-postgres-tcp-connection]] /
[[20260731_cloudflare-d1-fit-evaluation]]

---

## 1. SPAルーティングのフォールバック挙動

Cloudflare Pages 公式ドキュメント「Serving Pages」に明記されている挙動:

- トップレベルに `404.html` が**無い**場合、Pages は自動的に SPA として扱い、すべての
  未知パスを root（`/`）にマッチさせて `index.html` を返す。
- つまり **`_redirects` ファイルが無くても**、`404.html` を置いていない限り自動的に
  SPA フォールバックが働く。逆に `404.html` を置くと、この自動 SPA 挙動は無効化される。

本プロジェクト（`frontend/public/` 配下、`frontend/` 直下）には `_redirects` /
`_headers` / `404.html` / `wrangler.toml` のいずれも存在しない（2026-08-01時点）。
したがって現状のままデプロイしても `/play/:levelId` のような直接アクセス・リロードは
自動的に `index.html` へフォールバックされ、追加設定は不要と考えられる。

未確認点: `_worker.js` や Pages Functions を併用した場合にこの自動 SPA フォールバックが
どう変化するかは公式ドキュメントに明記が無い。

出典:
- https://developers.cloudflare.com/pages/configuration/serving-pages/
- https://developers.cloudflare.com/workers/static-assets/routing/single-page-application/

## 2. ビルド環境でのRustツールチェイン（最大の技術的ハードル）

Cloudflare Pages の公式 Build image ドキュメントの言語サポート一覧に **Rust は
掲載されていない**（掲載されているのは Go, Node.js, Bun, Python, Ruby, PHP, Java,
Clojure, Elixir, Erlang, Swift, .NET のみ）。

- ビルド環境は Ubuntu 22.04.2 / x86_64 の gVisor コンテナ。ビルドタイムアウトは
  20分（公式 Limits ページに明記）。
- Rust は非公式サポートだが、ビルドコマンド内で
  `curl https://sh.rustup.rs -sSf | sh` のように rustup を都度インストールして
  ビルドする運用はコミュニティで報告例がある（Cloudflare Community、Answer Overflow）。
  `rustup target add wasm32-unknown-unknown` や特定バージョン固定の
  `cargo install wasm-bindgen-cli --version 0.2.126`（本プロジェクトの
  `frontend/scripts/build-wasm.sh` が要求するバージョン）も同様にビルドスクリプト内で
  実行可能と考えられるが、**公式サポート外・動作保証外**であり、20分のタイムアウト
  制限内に収まるかは要検証。
- 代替案: CI（GitHub Actions等）またはローカルで事前に wasm ビルドを行い、生成物
  （`frontend/public/wasm/`, `frontend/public/assets/`。現状 gitignore 対象）を
  リポジトリにコミットしておき、Cloudflare Pages 側では公式サポート言語である
  Node.js のみで `vite build`（`tsc -b && vite build`）だけを実行する運用の方が、
  公式サポート範囲内で完結し安全である。

出典:
- https://developers.cloudflare.com/pages/configuration/build-image/
- https://developers.cloudflare.com/pages/platform/limits/
- https://community.cloudflare.com/t/support-for-leptos-for-cloudflare-pages/641931
- https://www.answeroverflow.com/m/1234538596107554816

## 3. WASMマルチスレッド（COOP/COEP）は現状不要

- `game_engine/Cargo.toml` の Bevy features 指定
  （`default-features = false, features = ["2d","ui","audio","png","jpeg","webp"]`）に
  `multi_threaded` は含まれない。
- `Cargo.lock` 全体にも `rayon` / `wasm-bindgen-rayon` / `atomics` の記述は無く、
  `.cargo/config.toml` にも threading 用 RUSTFLAGS
  （`-C target-feature=+atomics,+bulk-memory`）は設定されていない。
- `game_engine/src/injection.rs` にも SharedArrayBuffer / crossOriginIsolated / Worker
  関連の記述は無い。

結論: 現状は COOP/COEP ヘッダー設定は不要。将来 WASM マルチスレッド化する場合は、
Cloudflare Pages の `_headers` ファイルで以下のように設定可能（公式に headers ファイル
自体はサポートされているが、COOP/COEP 専用の公式サンプルは無いため一般的な `_headers`
構文からの類推）。

```
/*
  Cross-Origin-Opener-Policy: same-origin
  Cross-Origin-Embedder-Policy: require-corp
```

出典:
- https://developers.cloudflare.com/pages/configuration/headers/

## 4. .wasmファイルのMIMEタイプ

公式ドキュメントに `.wasm → application/wasm` の明示的なマッピング記載は見つからず。
Blazor(WASM) デプロイガイドでは追加設定不要とされる一方、Cloudflare Community には
`application/octet-stream` になったという個別報告もあり、断定はできない（未確認）。

念のため `_headers` で `Content-Type: application/wasm` を明示するのが安全策。

出典:
- https://developers.cloudflare.com/pages/framework-guides/deploy-a-blazor-site/
- https://community.cloudflare.com/t/hosting-content-on-cloudflare-pages-service-and-mime-types/259686

## 5. モノレポでのRoot Directory設定

リポジトリは `frontend/` と `game_engine/` が兄弟ディレクトリ。GitHub Issue
（cloudflare/workers-sdk #10941）で確認: Root Directory を設定しても、リポジトリ全体が
チェックアウトされディスク上に兄弟ディレクトリも存在する。

公式 Monorepos ドキュメントでは「Root Directory はビルドコマンドの実行場所を指定する
もの」とのみ記載。よって `frontend/` を Root Directory にしても、ビルドコマンド内で
`../game_engine` のような相対パス参照（`frontend/scripts/build-wasm.sh` が実際に
`ENGINE_DIR="$FRONTEND_DIR/../game_engine"` という相対パス解決をしている）は動作すると
考えられる（状況証拠ベースであり公式の明確な保証ではない）。

出典:
- https://developers.cloudflare.com/pages/configuration/monorepos/
- https://github.com/cloudflare/workers-sdk/issues/10941

## 6. 教訓：一次情報を確認する前に断定しない

一度目の回答では検証せず一般的な SPA ホスティングの知識から「`_redirects` が必須」と
断定してしまい誤りだった。`vite build` + `vite preview` でローカル検証したところ
再現せず、Cloudflare 公式ドキュメントを確認して初めて「`404.html` が無ければデフォルトで
SPA フォールバックする」という仕様が判明した。インフラ・ホスティングサービス固有の挙動は
一般論からの類推が外れることがあるため、断定する前に一次情報（公式ドキュメント）を
確認する必要がある。

## 関連ドキュメント

- [[20260731_cloudflare-pages-functions-postgres-tcp-connection]]（同じく Cloudflare
  Pages Functions のグレーゾーン・未確認事項を一次情報ベースで整理した調査）
- [[20260731_cloudflare-d1-fit-evaluation]]（Cloudflare エコシステム内の他サービスに
  ついての採否検討）
- [[20260801_cloudflare-pages-local-deploy-practice]]（実践編。本書の検討結果を踏まえて
  実際にデプロイした作業記録。理論通りだった点と、実際にやってみて初めて分かった点を対比）
