# ローカル開発環境（デプロイ）

## この文書について

全体像は [overview.md](./overview.md) を参照。本番のホスティング・CI/CD は
[hosting-and-cicd.md](./hosting-and-cicd.md) を参照。本文書はローカルで一式を
Docker Compose ベースで再現するための構成を扱う。

## 1. 各コンポーネントのローカルでの動かし方

| コンポーネント | ローカルでの動かし方 |
|---|---|
| Supabase（Auth/Postgres = ユーザー情報、Storage = 画像） | `supabase start`（Supabase CLI）を実行するだけで、本番相当のスタックが Docker 上に一括で立ち上がる。内部構成の詳細は CLI に任せ、意識しなくてよい |
| DynamoDB（ゲームデータ。[backend.md](./backend.md) §2） | `amazon/dynamodb-local`（AWS 公式 Docker イメージ）をポート 8000 で起動 |
| フロント（React/WASM） | `vite dev` |
| 自前 API 層（Cloudflare Pages Functions） | `wrangler pages dev`（接続先はローカルの Supabase スタック・DynamoDB Local を向ける） |

4つとも Docker（または Docker 相当のローカルプロセス）で完結し、追加のクラウド契約は不要。
Supabase CLI（Docker Compose ベース）と `amazon/dynamodb-local`（Docker）は、Supabase CLI が
管理するスタックとは別々に起動・連携させる必要がある。

## 2. 雛形（DynamoDB Local を追加する場合の差分）

```yaml
# docker-compose.yml （Supabase CLI 管理分とは別に、DynamoDB Local のみ追加する例）
services:
  dynamodb-local:
    image: amazon/dynamodb-local:latest
    command: ["-jar", "DynamoDBLocal.jar", "-sharedDb", "-dbPath", "/data"]
    ports:
      - "8000:8000"
    volumes:
      - dynamodb-data:/data

volumes:
  dynamodb-data:
```

実運用では、Supabase 側は `supabase init` → `supabase start` に任せ、生成される Docker
構成をプロジェクトの `supabase/` ディレクトリで管理するのが公式に推奨される方法であり、
保守コストが低い。上記の DynamoDB Local 部分だけ別途 compose に追加する形になる。
