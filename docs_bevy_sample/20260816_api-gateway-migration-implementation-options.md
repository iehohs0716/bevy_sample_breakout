# APIゲートウェイ移行の実装差分イメージ（パターン別）

作成日: 2026-08-16
関連: `docs_bevy_sample/20260816_api-gateway-nginx-kong-envoy-tradeoff-analysis.md`（判断材料の分析はこちらを参照。本ドキュメントは実装差分のみを扱う）

## 前提

本ドキュメントは、分析レポートで整理した3パターン（nginx現状維持／Kong移行／Envoy移行）について、
実際にコードを変更する場合の具体的な差分イメージを示す。**現時点ではどのパターンも実装していない**
（今回はレポート作成のみがスコープ）。

## パターンA: 現状維持（nginxのまま）

変更対象ファイル: なし。

**今回このパターンを選ぶ場合の記録**: `supabase-local/volumes/auth-gateway/nginx.conf`のCORS関連の
3バグ（①OPTIONSにヘッダー無し、②重複、③固定リスト漏れ）は既に解消済み。今のスコープ（GoTrueのみ）
に厳密に一致させたいならこのまま変更しない。

## パターンB: Kongへ移行する場合

### 変更対象ファイル

- 削除: `docker-compose.yml`の`auth-gateway`サービス定義
- 削除: `supabase-local/volumes/auth-gateway/`ディレクトリ（`nginx.conf`）
- 新規: `supabase-local/volumes/api/kong.yml`
- 変更: `docker-compose.yml`に`kong`サービスを追加

### `docker-compose.yml`差分イメージ

```yaml
  kong:
    container_name: supabase-kong
    image: kong:3.9-alpine
    restart: unless-stopped
    ports:
      - "8002:8000"
    environment:
      KONG_DATABASE: "off"
      KONG_DECLARATIVE_CONFIG: /var/lib/kong/kong.yml
      KONG_DNS_ORDER: LAST,A,CNAME
      KONG_PLUGINS: bundled
    volumes:
      - ./supabase-local/volumes/api/kong.yml:/var/lib/kong/kong.yml:ro,Z
    depends_on:
      auth:
        condition: service_healthy
```

### `kong.yml`差分イメージ（authルートのみの最小構成）

```yaml
_format_version: "2.1"
_transform: true

services:
  - name: auth-v1
    url: http://auth:9999/
    routes:
      - name: auth-v1-all
        strip_path: true
        paths:
          - /auth/v1/
    plugins:
      - name: cors   # origins/headers等は明示せず、Kongのデフォルト動的挙動に委ねる
```

（公式`docker/volumes/api/kong.yml`にある`auth-v1-open*`系ルート—verify/callback/authorize等の
個別公開ルート—が必要かどうかは、フロント側のOAuthフロー実装に応じて追加検討が必要）

### CORSバグの再発可能性

- ①（OPTIONSヘッダー無し）: Kongが3条件成立時に自己応答するため、GoTrueの未解明挙動から切り離される。再発しにくい見込みだが実機検証は必要。
- ②（重複）: `set_header`一元管理が標準動作のため再発しない。
- ③（固定リスト漏れ）: `headers`未指定＝動的反映がデフォルトのため再発しない。

## パターンC: Envoyへ移行する場合

分析レポート§2補足の通り、「最小構成に絞る」制約を外し、**公式テンプレートをほぼそのまま流用する**
方針で書く。この方が「7クラスタ→1クラスタに削る」作業より手間が少なく、公式の動作確認済み構成を
そのまま使えるためミスが起きにくい。

### 変更対象ファイル

- 削除: `docker-compose.yml`の`auth-gateway`サービス定義
- 削除: `supabase-local/volumes/auth-gateway/`ディレクトリ（`nginx.conf`）
- 新規: `supabase-local/volumes/api/envoy/envoy.yaml`（公式テンプレートをそのままコピー）
- 新規: `supabase-local/volumes/api/envoy/cds.yaml`（公式テンプレートをそのままコピー。
  auth以外の6クラスタ定義は残したままでよい。実体のないアップストリームはヘルスチェックが
  失敗し続けるだけで、authクラスタへのルーティングには影響しない）
- 新規: `supabase-local/volumes/api/envoy/lds.template.yaml`（公式テンプレートをそのままコピー）
- 新規: `supabase-local/volumes/api/envoy/docker-entrypoint.sh`（公式テンプレートをそのままコピー）
- 変更: `docker-compose.yml`に`envoy`サービスを追加
- 変更: ルートの`.env`/`.env.example`に、Envoyの`docker-entrypoint.sh`が参照する環境変数を追加
  （下記参照）

### `docker-compose.yml`差分イメージ

```yaml
  envoy:
    container_name: supabase-envoy
    image: envoyproxy/envoy:v1.31-latest
    restart: unless-stopped
    ports:
      - "8002:8000"
    environment:
      ANON_KEY: ${ANON_KEY}
      SERVICE_ROLE_KEY: ${SERVICE_ROLE_KEY}
      # ANON_KEY_ASYMMETRIC / SERVICE_ROLE_KEY_ASYMMETRIC / SUPABASE_PUBLISHABLE_KEY /
      # SUPABASE_SECRET_KEY は今回未設定のまま。docker-entrypoint.sh の実装上、
      # これらが揃わない場合は「レガシーAPIキーモード」で動作する（フル機能ではないが
      # 今回のGoogleログイン検証用途には支障がない見込み）。
      DASHBOARD_USERNAME: ${DASHBOARD_USERNAME:-supabase}
      DASHBOARD_PASSWORD: ${DASHBOARD_PASSWORD:-this_password_is_insecure_and_should_be_updated}
    volumes:
      - ./supabase-local/volumes/api/envoy:/etc/envoy:ro,Z
    entrypoint: ["/etc/envoy/docker-entrypoint.sh"]
    depends_on:
      auth:
        condition: service_healthy
```

### 環境変数の追加（ルートの`.env.example`）

```dotenv
DASHBOARD_USERNAME=supabase
DASHBOARD_PASSWORD=this_password_is_insecure_and_should_be_updated
```

### CORSバグの再発可能性

- ①（OPTIONSヘッダー無し）: EnvoyのCORSフィルタがroute単位でプリフライトに自己応答する設計のため、GoTrueの未解明挙動から完全に切り離される。最も再発しにくい。
- ②（重複）: 公式テンプレートでCORSヘッダーが一元管理されている前提であれば再発しない（実装の詳細確認は別途必要）。
- ③（固定リスト漏れ）: `allow_headers: "*"`のワイルドカードのため、「リストに漏れる」という事象自体が構造上発生しない。再発可能性は最も低いが、セキュリティ的には最も緩い設定である点に留意（ローカル検証用途なので許容範囲）。

### 未検証・要確認事項

- 実体のないクラスタ（rest以外、あるいはrest含む全て）へのヘルスチェック失敗が、Envoyコンテナ自体の
  健全性（`healthy`判定）やログ量にどの程度影響するかは実機未検証。導入時に確認する。
- `lds.template.yaml`のルーティングは`rest`等が別ポート（今回は`8001`で直接公開）で稼働している
  前提と衝突しないか、実際に導入する際にポート構成を再確認する必要がある。

## 総合見解（再掲）

- ゲートウェイ設定を今のスコープに厳密に一致させたいならパターンA（現状維持）。
- 将来のフルスタック化を見据えるならパターンC（Envoy、公式テンプレートをほぼそのまま流用）。
- パターンB（Kong）は、公式が新規採用を非推奨としているため、いずれの前提でも積極的に選ぶ理由がない。
