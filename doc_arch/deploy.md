# ローカル開発環境（デプロイ）

## この文書について

全体像は [overview.md](./overview.md) を参照。本番のホスティング・CI/CD は
[hosting-and-cicd.md](./hosting-and-cicd.md) を参照。本文書はローカルで一式を
Docker Compose ベースで再現するための構成を扱う。

## 1. 各コンポーネントのローカルでの動かし方

| コンポーネント | ローカルでの動かし方 |
|---|---|
| Supabase（Auth/Postgres = ユーザー情報、Storage = 画像） | `docker compose up -d`（リポジトリ直下の`docker-compose.yml`を手動管理）。Supabase公式のセルフホストフルスタック構成（`db`/`rest`/`auth`/`api-gw`(Envoy)/`realtime`/`storage`/`imgproxy`/`meta`/`studio`/`functions`/`supavisor`の11サービス）を再現している。Supabase CLIの`supabase start`は使わない（詳細は§3） |
| DynamoDB（ゲームデータ。[backend.md](./backend.md) §2） | `amazon/dynamodb-local`（AWS 公式 Docker イメージ）をポート 8000 で起動。**注意**: Envoy(`api-gw`)も既定でポート8000を使うため、DynamoDB Localを追加する際はどちらかのポートを変更する必要がある |
| フロント（React/WASM） | `vite dev` |
| 自前 API 層（Cloudflare Workers） | `wrangler dev`（接続先はローカルの Supabase スタック・DynamoDB Local を向ける） |

4つとも Docker（または Docker 相当のローカルプロセス）で完結し、追加のクラウド契約は不要。

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

上記の DynamoDB Local 部分は、リポジトリ直下の `docker-compose.yml`（§3）に統合する形で
追加する。

## 3. Supabaseローカルスタックの構成（フルスタック11サービス）

学習・検証目的のため、本番で使わない予定のサービス（Realtime / Edge Functions / Supavisor）
を含め、Supabase公式のセルフホストDocker Compose構成を**フルスタックでローカルに再現する**。
本番の[backend.md](./backend.md)§3・§4の方針（Realtime/Edge Functions/Supavisorは使わない、
フロントはPostgREST/Storage SDKを直接叩かない）とは独立した、ローカル環境固有の判断である。

- ゲートウェイはKongではなくEnvoy（Supabase公式が2026年8月からセルフホストのデフォルトに
  変更したもの）を採用する。詳細は
  `docs_bevy_sample/20260816_api-gateway-nginx-kong-envoy-tradeoff-analysis.md`。
- 公式のCompose定義・Envoy設定（`cds.yaml`等の7クラスタ）・db初期化SQLはそのまま流用する
  （最小構成に削る作業はしない）。
- Studioダッシュボードは`http://localhost:8000/`（Envoy経由、Basic認証）からアクセスする。
- `rest`（PostgREST）は動作確認の利便性のため`http://localhost:8001`にも直接公開しており、
  こちらはEnvoyのapikey必須ルールを経由しない。
- Supavisorは`localhost:5432`（セッションモード）・`localhost:6543`（トランザクションモード）
  で直接検証できるが、`rest`/`auth`の接続経路には使わない（本番方針を局所的に維持。詳細は
  [backend.md](./backend.md)§4.5）。
