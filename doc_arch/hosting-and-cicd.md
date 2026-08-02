# ホスティング・CI/CD

## この文書について

全体像は [overview.md](./overview.md) を参照。ローカル開発環境は [deploy.md](./deploy.md) を参照。

## 1. ホスティング・CI/CD 構成

- フロント + バックエンド（自前 API 層）: 同一の Cloudflare Workers プロジェクトとして配信する。
  Workers Static Assets で静的アセット（React + WASM）を配信しつつ、`run_worker_first` で
  `/api/*` のみ Worker 側の自前 API ロジックに振り分ける（[backend.md](./backend.md) §3）。
  ソースは `frontend/`（静的アセットのビルド元）と `worker/`（Worker 本体。`wrangler.jsonc` の
  `assets.directory` が `frontend/dist` を参照）の 2 ディレクトリに分かれる。

- **デプロイは Cloudflare ダッシュボードの Git 連携（Workers Builds）ではなく、GitHub Actions
  から `wrangler deploy` を実行する形で行う。** Workers Builds は Worker 1 つにつき
  Root directory・Build command・Deploy command をそれぞれ 1 つしか設定できず、`frontend/` と
  `worker/` という別ディレクトリをまたいだビルド（`frontend/` を先にビルドしてから `worker/`
  からデプロイする）を公式にサポートすると明記した一次情報が見つからなかったため。GitHub Actions
  であれば複数ステップの shell コマンドを自由に書けるため、この制約を受けない。

- CI/CD（GitHub Actions）:
  - PR 時（検証のみ、デプロイしない）: `cargo check --target wasm32-unknown-unknown` →
    `cd frontend && pnpm build:wasm && pnpm build`（`tsc -b` → `vite build` を含む）を実行し、
    ビルドが通ることを確認する
  - `main` ブランチへの push 時（本番デプロイ）: 上記に加えて `cd worker && npx wrangler deploy`
    を実行し、`frontend/dist` を含めて 1 つの Cloudflare Worker として本番デプロイする
- `wasm-bindgen-cli` のバージョンを `Cargo.lock` と一致させる手順を CI にも明記（既存の
  `CLAUDE.md` に記載のローカル制約と同じ）
- Supabase: `supabase db push` によるマイグレーション適用を CI/CD パイプラインに組み込む
