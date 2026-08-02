# バックエンドをCloudflare Workers単体で新設する場合の実装調査（Supabaseユーザー CRUD / DynamoDBゲームデータ CRUD）

日付: 2026-08-02

## 0. 位置づけ

今回の方針: 自前API層（Facade）を、フロントとは別のCloudflare Workersプロジェクトとして新設する。
ユーザーCRUDはSupabase（Postgres）、ゲームデータCRUDはDynamoDBに対して行う。

`doc_arch/overview.md` / `doc_arch/backend.md` はすでに以下を確定済みであり、本書はこれを変更しない。

- Supabase（Auth・ユーザー情報）＋ DynamoDB（ゲームデータ）のハイブリッド構成
- フロントはSupabase/DynamoDBを直接叩かず、自前API層としか話さない（Facadeパターン）
- 認可判定は自前API層のコードで行う。RLSは保険程度

一方、`doc_arch/backend.md` §3 は自前API層を「フロントと同じ Cloudflare Workers 上にデプロイでき
（Workers Static Assets で静的アセットも配信）」と書いており、フロントとバックエンドを**同一の
Worker**に同居させる想定になっている。今回の「Workers単体で新設」はこれと異なり、フロントとは
**別のWorkerプロジェクト**としてバックエンドを立てる方針である。この差分は §8 の未決事項として
明記し、`doc_arch` 側への反映は別途行う。

本書は実装方式のWeb調査であり、実際にコードを書いて動作確認したものではない（ドキュメント調査）。

## 1. 全体構成案

- 新規ディレクトリ（例: `backend/`。`game_engine/` / `frontend/` と並列）に、フロントとは独立した
  wranglerプロジェクトを作る。
- ルーティング: **Hono**。Cloudflare公式ドキュメントに動作確認済みフレームワークとして掲載されており
  ([Hono · Cloudflare Workers docs](https://developers.cloudflare.com/workers/framework-guides/web-apps/more-web-frameworks/hono/))、
  2026年時点でCloudflare Workers上のAPI構築における事実上の標準になっている。WebCrypto/Fetch API
  ベースで動くため`nodejs_compat`なしでも動作する。
- エンドポイント例（`doc_arch/backend.md` §6 の契約を踏襲、パスは仮）:
  - ユーザー: `GET/POST/PATCH/DELETE /api/users/...`（Supabase Postgres）
  - ゲームデータ: `GET/POST/PATCH/DELETE /api/scenarios/...`（DynamoDB）

## 2. CORS: フロントとバックエンドが別オリジンになる

同一Workerに同居させる案と違い、別Workerに分離すると呼び出しは別オリジン間のHTTPリクエストになり、
CORS対応が必須になる。

- Honoの[cors組み込みミドルウェア](https://hono.dev/docs/middleware/builtin/cors)を全ルートに適用する。
- `origin` はフロントの本番オリジン・ローカル開発オリジンを明示的に列挙する。ワイルドカード
  （`*`）は使わない — これは `doc_arch/backend.md` §7 が Supabase Storage バケットの CORS 設定に
  ついてすでに定めている「ワイルドカード許可はしない」という既存方針と揃える。

## 3. 認証: SupabaseのJWTをWorkers側で検証する

- Supabase Authは2025年5月1日以降に作成されたプロジェクトでは**デフォルトでRS256（非対称鍵）**を
  使う（[Supabase Auth: Asymmetric Keys support](https://supabase.com/changelog/29289-supabase-auth-asymmetric-keys-support-in-2025)）。
  公開鍵はJWKSエンドポイント `https://<project_ref>.supabase.co/auth/v1/.well-known/jwks.json`
  で配布される（[JWT Signing Keys | Supabase Docs](https://supabase.com/docs/guides/auth/signing-keys)）。
- Workers側では`jose`ライブラリの`createRemoteJWKSet` + `jwtVerify`でこのJWKSを取得・検証する。
  `jose`はWebCrypto APIベースで実装されており、Cloudflare Workersを公式にサポート対象ランタイムと
  明記している（[panva/jose](https://github.com/panva/jose)）。`nodejs_compat`は不要。
- この構成では、バックエンドはSupabaseのSDK（`@supabase/supabase-js`）に一切依存せず、標準的な
  JWT検証だけで認可判定ができる。`doc_arch/backend.md` §3 が課題として挙げている「JWT発行元が
  Supabase固有」というロックインは、JWKS URLを差し替えるだけで発行元を切り替えられる形になり、
  §3の「発行元（Issuer）を差し替え可能な作りにする」という方針をそのまま満たす。
- 旧プロジェクト（HS256・対称鍵）の場合は共有シークレットでの検証になる。対象Supabaseプロジェクトが
  RS256かHS256かは、実装着手時に `/auth/v1/.well-known/jwks.json` のレスポンス内容を見て確認する
  必要がある。

## 4. ユーザーCRUD: Supabase Postgresへの接続

既存調査 `docs_bevy_sample/20260731_cloudflare-pages-functions-postgres-tcp-connection.md` の
結論をそのまま踏襲できる。要点の再掲:

- **Hyperdriveバインディング経由が公式に明確サポートされた接続経路**。素の`cloudflare:sockets`
  直結はレイテンシ面で公式に非推奨とされている。
- Hyperdriveはそもそも Cloudflare Workers 向けに作られた機能で、Pages Functions はそこに間借り
  する形で対応が案内されている。今回のように**フロントと分離した独立Workerでバックエンドを組む
  構成は、Hyperdriveの本来の主要ユースケースそのもの**であり、前回調査でPages Functions固有の
  懸念点として残っていた「Pages Functions上でTCP Sockets APIやHyperdriveが確実に動くか未確認」
  というグレーゾーンがそもそも発生しない。
- `wrangler.toml`（または`wrangler.json`）に`[[hyperdrive]]`でSupabaseのDirect connection文字列を
  登録し、`pg`（node-postgres, v8.16.3以上）経由でSQLを発行する
  （[Connect to a PostgreSQL database with Cloudflare Workers](https://developers.cloudflare.com/workers/tutorials/postgres/)）。
  `nodejs_compat`フラグ（`compatibility_date`が2024-09-23以降）が必要。
- 認可判定はAPI層のコードで行い、RLSは保険程度に留める（既存方針どおり、変更なし）。

## 5. ゲームデータCRUD: DynamoDBへの接続

- **選択肢A: AWS SDK v3をフル採用**。Cloudflare公式テンプレート
  [`cloudflare/workers-aws-template`](https://github.com/cloudflare/workers-aws-template)が
  DynamoDB/SQSアクセスの実装例を提供している。`nodejs_compat`フラグが必要。認証情報は
  `wrangler secret put AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY`で管理し、SigV4署名はSDKが
  内部で行う。
- **選択肢B: `aws4fetch`等の軽量SigV4署名ライブラリ**で、DynamoDBのHTTP APIを`fetch`から直接叩く。
  バンドルサイズ・コールドスタートで有利だが、リクエストの組み立てを自前で書く必要がある。
- どちらもWorkers単体で完結する。選択肢Aの場合、§4のPostgres接続と同じ`nodejs_compat`フラグを
  共有できる。

## 6. 依存関係まとめ

| 機能 | 主な依存 | `nodejs_compat`要否 |
|---|---|---|
| ルーティング・CORS | Hono | 不要 |
| JWT検証（Supabase Auth） | jose | 不要（WebCrypto） |
| ユーザーCRUD（Supabase Postgres） | Hyperdriveバインディング + pg | 必要 |
| ゲームデータCRUD（DynamoDB） | AWS SDK v3、または aws4fetch | AWS SDK v3のみ必要。aws4fetchなら不要 |

## 7. ローカル開発への影響

`doc_arch/deploy.md` はすでに「自前API層（Cloudflare Workers）は`wrangler dev`で、ローカルの
Supabaseスタック・DynamoDB Localを向ける」という構成を記載済みであり、フロントとバックエンドを
別Workerに分けても、この記述自体に変更は生じない（`wrangler dev`を2プロセス起動する形になる程度の
差分）。

## 8. 未決事項・要確認

- フロント／バックエンドを別Workerに分離する方針を `doc_arch/backend.md` §3・`doc_arch/overview.md`
  に反映するかどうか。現状の記述は「フロントと同じCloudflare Workers上にデプロイ」のままになっている。
- Hyperdriveの料金体系・対応リージョン、Supabase側Direct connectionとの組み合わせでの実測レイテンシは
  実装着手時に個別検証が必要（既存未決事項のまま、変更なし）。
- AWS SDK v3 と aws4fetch のどちらを採用するか（バンドルサイズ・実装コストのトレードオフ）は未決定。
- 対象のSupabaseプロジェクトがRS256（新方式）かHS256（旧方式）かは、実装着手時に
  `/auth/v1/.well-known/jwks.json`で確認する必要がある。

## Sources

- [Hono · Cloudflare Workers docs](https://developers.cloudflare.com/workers/framework-guides/web-apps/more-web-frameworks/hono/)
- [CORS Middleware - Hono](https://hono.dev/docs/middleware/builtin/cors)
- [JWT Signing Keys | Supabase Docs](https://supabase.com/docs/guides/auth/signing-keys)
- [Supabase Auth: Asymmetric Keys support in 2025 · Changelog](https://supabase.com/changelog/29289-supabase-auth-asymmetric-keys-support-in-2025)
- [JavaScript: getClaims | Supabase Docs](https://supabase.com/docs/reference/javascript/auth-getclaims)
- [panva/jose — GitHub](https://github.com/panva/jose)
- [Connect to a PostgreSQL database with Cloudflare Workers](https://developers.cloudflare.com/workers/tutorials/postgres/)
- [cloudflare/workers-aws-template — GitHub](https://github.com/cloudflare/workers-aws-template)

## 関連ドキュメント

- `doc_arch/overview.md` / `doc_arch/backend.md` / `doc_arch/deploy.md`（既存の確定事項。本書は
  「バックエンドを別Workerとして新設する」場合の実装方式を補足するもので、確定事項自体は変更しない）
- `docs_bevy_sample/20260731_cloudflare-pages-functions-postgres-tcp-connection.md`（Hyperdrive経由の
  Postgres接続調査。本書§4の元ネタ）
- `docs_bevy_sample/20260731_cloudflare-d1-fit-evaluation.md`（D1不採用の検討。本書とは独立）
- `docs_bevy_sample/20260802_cloudflare-storage-products-comparison.md`（Cloudflare自社ストレージ製品比較）
