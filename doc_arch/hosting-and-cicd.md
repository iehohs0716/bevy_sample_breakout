# ホスティング・CI/CD

## この文書について

全体像は [overview.md](./overview.md) を参照。ローカル開発環境は [deploy.md](./deploy.md) を参照。

## 1. ホスティング・CI/CD 構成

- フロント: Cloudflare Pages（Git 連携で push 時自動デプロイ）
- ビルド: `pnpm build:wasm` → `tsc -b` → `vite build`（既存の `frontend/package.json` の
  `build` スクリプトをそのまま CI で実行）
- CI: GitHub Actions で `cargo check --target wasm32-unknown-unknown` → `pnpm build` を実行し、
  Cloudflare Pages の Git 連携ビルドと同じ手順を PR 時にも検証する
- `wasm-bindgen-cli` のバージョンを `Cargo.lock` と一致させる手順を CI にも明記（既存の
  `CLAUDE.md` に記載のローカル制約と同じ）
- Supabase: `supabase db push` によるマイグレーション適用を CI/CD パイプラインに組み込む
