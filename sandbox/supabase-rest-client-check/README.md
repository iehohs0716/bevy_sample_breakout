# supabase-rest-client-check

ローカルの Supabase（`db` + `rest`(PostgREST) の最小構成）に対する疎通確認ツール。
Supabase Auth（GoTrue）は経由せず、**anon key だけを使ってPostgRESTのREST APIを直接叩く**方式で、
`poc_check` テーブルへの INSERT / SELECT が実際に反映されるかを確認する。

## 構成

- `check_connectivity.py` — 実際にHTTPリクエストを送るクライアント本体（Python, `uv run` で実行）
- `run.sh` — Docker起動 → healthy待機 → INSERT/SELECT確認 を一気通貫で行うラッパー

`poc_check` テーブル自体はこのディレクトリのコードでは作成しない。リポジトリルートの
`supabase-local/volumes/db/poc_check.sql` が、`db` コンテナの初回起動時
（`docker-entrypoint-initdb.d` 経由、マウント設定は `docker-compose.yml` 参照）に自動作成する。

## 前提条件

- `docker` / `docker compose`（`db` + `rest` のローカルスタック起動用）
- `uv`（`check_connectivity.py` の実行用。インストール: `curl -LsSf https://astral.sh/uv/install.sh | sh`）
- リポジトリルートに `.env` が存在すること（無ければ `.env.example` をコピーして作成）

```sh
cp .env.example .env   # リポジトリルートで、まだ .env が無い場合のみ
```

## 使い方

### 一括実行（推奨）

```sh
./run.sh
```

`db` + `rest` を起動し、healthyになるのを待ってから `poc_check` へ1行INSERTし、
続けて全件SELECTして結果を表示する。停止方法も最後に案内される。

### 個別に叩く場合

```sh
# リポジトリルートで db + rest を起動
docker compose up -d db rest

# このディレクトリで
./check_connectivity.py insert --label "手動確認1"
./check_connectivity.py select
```

`--rest-url`（既定: `http://localhost:8001`）・`--env-file`（既定: リポジトリルートの `.env` を自動探索）
はオプションで上書き可能（`./check_connectivity.py --help` 参照）。

## 停止・データの初期化

```sh
docker compose down       # 停止のみ。poc_check の中身は volume に残る
docker compose down -v    # volumeごと削除。次回起動時は poc_check テーブルからまっさらに再作成される
```

## トラブルシューティング

- **`rest` が healthy にならない**: `docker compose logs rest` を確認。
  `.env` の `PGRST_DB_SCHEMAS` に、実際には存在しないスキーマ（例: 使っていない `graphql_public`）が
  含まれていると、PostgREST がスキーマキャッシュの読み込みに失敗し続けて `unhealthy` から
  回復しない。このプロジェクトでは既に `PGRST_DB_SCHEMAS=public` のみにしてある。
- **`relation "public.poc_check" does not exist`**: `poc_check.sql` は volume の**初回作成時のみ**
  実行される（Postgresの `docker-entrypoint-initdb.d` の一般仕様）。既存の volume が残ったまま
  `poc_check.sql` を追加・変更しても反映されないので、`docker compose down -v` で volume ごと
  作り直すこと。
- **`.env` が見つからない**: リポジトリルート直下に `.env` が無い。「前提条件」参照。
