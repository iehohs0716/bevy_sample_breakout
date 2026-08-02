# 自前API層のCloudflare Workers実装方式に関する意思決定の経緯

日付: 2026-08-02

## 0. 位置づけ

本ドキュメントは、`doc_arch/overview.md`・`doc_arch/backend.md`ですでに確定していた
「Supabase（Auth・ユーザー情報）＋DynamoDB（ゲームデータ）のハイブリッド構成」「自前API
レイヤー（Facade）でフロントからSupabase/DynamoDBを隠蔽する」という方針を前提に、その
自前APIレイヤーを**実際にCloudflare Workers上にどう構築するか**を詰めた一連の議論の、
**経緯そのもの**（最終的にどう決着したか、途中でどう方針転換したか、なぜそうなったか）を
記録するものである。

個別トピックの技術詳細（実装調査・API・コード例）は以下3ファイルに譲り、本書はリンクを
貼るのみで内容を重複させない。

- `docs_bevy_sample/20260802_standalone-workers-backend-supabase-dynamodb-crud.md`
- `docs_bevy_sample/20260802_hono-requirement-for-same-worker-frontend-backend.md`
- `docs_bevy_sample/20260802_google-sso-feasibility-with-jwks-verification.md`

## 1. 出発点

`doc_arch/overview.md`・`doc_arch/backend.md`はすでに「Supabase（Auth・ユーザー情報）＋
DynamoDB（ゲームデータ）のハイブリッド構成」「自前APIレイヤー（Facade）でフロントから
Supabase/DynamoDBを隠蔽する」という方針を確定済みだった。今回の議論は、この自前APIレイヤーを
実際にCloudflare Workers上にどう構築するかを詰めるものだった。

## 2. 「別Worker」への一旦の決定

バックエンドを「フロントとは別の独立したCloudflare Workersプロジェクト」として新設する方針で
最初検討し、Web調査（Hyperdrive経由のPostgres接続、jose+JWKSによるJWT検証、DynamoDBへの
AWS SDK v3/aws4fetch接続）を行った上で、`doc_arch/backend.md` §3・`doc_arch/overview.md`・
`doc_arch/hosting-and-cicd.md`を「フロントとは別Workerに分離する」という決定でいったん
更新した（この時の実装調査が
`docs_bevy_sample/20260802_standalone-workers-backend-supabase-dynamodb-crud.md`）。

## 3. 「同一Workerでも技術的に可能」という訂正

ユーザーから「Reactフレームワークを使っている場合、フロントとバックを同一Workerに同居させる
ことはできないのでは」という疑問が出たが、調査の結果これは誤りで、Cloudflare公式ブログ
「Your frontend, backend, and database — now in one Cloudflare Worker」の通り、Workers Static
Assetsの`run_worker_first`（例: `["/api/*"]`）でパスを振り分けることで、1つのCloudflare
Workerが静的配信とAPIの両方を担う構成が公式にサポートされていることが判明した。これにより
「別Worker」「同一Worker」はどちらも技術的に実現可能な選択肢であり、CORSの要否・デプロイの
独立性がトレードオフになることを整理した。

## 4. セキュリティ懸念の解消

ユーザーから「同一Workerにすると、devtoolsで接続情報（DB接続文字列・AWSキー等）が見えて
しまうのでは」という懸念が出た。調査の結果、Cloudflare Workerはサーバーサイドで実行される
ため、`wrangler secret`やbinding経由の秘密情報はブラウザに一切送信されないこと、同一Worker/
別Workerのどちらを選んでもこの点は変わらないことを確認した。実際に情報が漏れるとしたら、
Viteの`VITE_`接頭辞環境変数をフロントコードで参照してしまうような実装ミスが原因であり、
Worker構成の選択とは無関係であることを整理した（本リポジトリは`doc_arch/frontend.md` §3で、
フロントが使ってよいのはSupabase Auth JS SDK（公開が前提のanon keyのみ使用）に限定しており、
service_role key・DB接続文字列・AWSキーはフロントのコードに一切登場しない設計になっている）。

## 5. 最終決定: 同一Worker構成を採用

セキュリティ上の懸念が解消されたことを受け、ユーザーは「簡単な同一Workerで動かす方針」を
最終決定した。これに伴い、`doc_arch/backend.md` §3・§7、`doc_arch/overview.md`（確定事項
テーブルおよび全体構成図）、`doc_arch/hosting-and-cicd.md`を、「フロントと自前API層は同一の
Cloudflare Workerプロジェクトに同居し、`run_worker_first`で`/api/*`のみWorker側に振り分ける。
CORS対応は不要」という内容に更新した（このときのやり取りの中で、「`run_worker_first`に
一致しないリクエストはWorker本体を経由せずWorkers Static Assetsが直接処理するため、Worker側
コードに`env.ASSETS.fetch()`のフォールバックは不要」という点も確認・反映済み）。

## 6. Honoフレームワークの要否

ユーザーから「同一Worker構成にすると、Honoフレームワークへの移管が必要になるのか」という
懸念が出たため調査した。結論は「必須ではない」。Cloudflare公式ドキュメントの
`run_worker_first`実装例自体、Honoを使わないプレーンな`fetch(request, env)`だけで完結して
おり、Honoは数あるルーティングライブラリの選択肢の一つに過ぎない。また、Hono（Worker側の
サーバーHTTPルーティング）とreact-router-dom（ブラウザ内のクライアントルーティング）は
完全に独立したレイヤーであり、一方の採用が他方に影響することもない。この結論は同一Worker案・
別Worker案のどちらでも変わらない（詳細は
`docs_bevy_sample/20260802_hono-requirement-for-same-worker-frontend-backend.md`）。
ただし、この後の会話でユーザーに見せた具体的なコードイメージはHonoを使う想定で書かれており、
Hono自体の採用可否は「使ってよい選択肢」として提示されたのみで、正式決定はしていない。

## 7. Auth0に関する誤解の訂正

ユーザーから「Supabase Auth+Postgresを使うな、Auth0を使うんじゃなかったのか」という指摘が
あったが、これは事実誤認だった。`docs_bevy_sample/20260731_auth0-supabase-third-party-auth.md`
は「Auth0の採用自体は未決定であり、あくまで選択肢の一つとして扱ったに過ぎない」と明記して
おり、`doc_arch/backend.md` §4のAuth.js検討も同様に「PoC未実施・採用は未確定」。
`doc_arch/overview.md`の確定事項テーブルは最初から一貫して「Supabase（Postgres + Auth）」で
あり、これは変更されていないことを説明した。

## 8. Google SSOの実現可否確認

ユーザーから「GoogleでSSOはできるのか」という質問があり調査した。結論は「確定済みの
Supabase Auth＋自前APIレイヤーでのJWKS検証というアーキテクチャの上で、追加の設計変更なしに
Google SSOは実現可能」（詳細・訂正込みの経緯は
`docs_bevy_sample/20260802_google-sso-feasibility-with-jwks-verification.md`。調査時点で
「PKCEフローがデフォルトで有効」という誤った説明をしてしまい、後に`@supabase/auth-js`
（`GoTrueClient`）の`DEFAULT_OPTIONS`のソースコードを確認して「`flowType`のデフォルトは
`'implicit'`であり、PKCEを使うには明示的に`flowType: 'pkce'`を指定する必要がある」と
訂正した経緯も含む）。

## 9. FSDとWorker側コードの関係の整理

ユーザーから「(Honoを使ったAPI層のコード構成は)FSDと共存できるのか」という質問があった。
調査の結果、Feature-Sliced Design(FSD)は公式ドキュメント（feature-sliced.design）で
「You're doing frontend」と明記されている通り**フロントエンド専用**の方法論であり、
バックエンドAPIサーバーへの適用は想定されていないことを確認した。したがって「共存できるか」
ではなく、そもそもWorker側のAPIコード（ルーティング・ミドルウェア等）はFSDの適用対象外で
あり、`frontend/src/`（FSD管理下）と物理的に分離すればよい、という整理になった。

## 10. 最終的なディレクトリ構成の決定

上記を踏まえ、以下のディレクトリ構成を提案しユーザーが合意した。

```
bevy_sample/
├── game_engine/     # Bevy(Rust/WASM)
├── frontend/        # React(Vite) — src/配下はFSD
│   ├── src/         # FSD (app/pages/widgets/entities)
│   └── dist/        # ビルド成果物 → worker/wrangler.jsoncのassets.directoryが指す
└── worker/          # Cloudflare Worker本体(自前API層) — FSD対象外
    ├── src/
    │   ├── index.ts
    │   ├── middleware/
    │   └── routes/
    └── wrangler.jsonc   # main: "./src/index.ts", assets.directory: "../frontend/dist"
```

ソースコードのディレクトリは分かれているが、`worker/wrangler.jsonc`が`assets.directory`で
`frontend/dist`を参照し、`worker/`から`wrangler deploy`を1回実行することで、両者は
**1つの共通Cloudflare Workerプロジェクト**としてデプロイされる（ソースの置き場所とデプロイ
単位は別の話であるという整理）。

## 11. CI/CDでのデプロイ方式の決定

上記のディレクトリ分離構成について、「Cloudflareダッシュボードの Git 連携（Workers
Builds）で自動デプロイできるか」を調査した。判明した事実（一次情報で確認済み）:

- `wrangler.jsonc`の`assets.directory`が設定ファイル自身の場所を基準にした相対パス解決で
  あり、`../frontend/dist`のような越境参照そのものはWrangler本体の仕様として問題ない
  （Wranglerのソースコード`packages/wrangler/src/assets.ts`で確認）。
- しかし、Cloudflare Workers Builds（ダッシュボードのGit連携機能）は、Workerプロジェクト
  1つにつきRoot directory・Build command・Deploy commandをそれぞれ1つしか設定できない
  （[Workers Builds Configuration](https://developers.cloudflare.com/workers/ci-cd/builds/configuration/)
  で確認）。モノレポ向けの説明
  （[Advanced setups](https://developers.cloudflare.com/workers/ci-cd/builds/advanced-setups/)）は
  あるが、そこで示されている例は独立した複数のWorkerサービスを1つのモノレポにまとめる
  ケースであり、「1つのWorkerが別ディレクトリのフロントエンドビルド成果物を使う」という
  今回のケースを公式にサポートすると明記した記述は見つからなかった。
- この点について、一度「`worker/`のBuild commandで
  `cd ../frontend && pnpm install && pnpm build`のように回避できる」と推測ベースで説明したが、
  これはCloudflare公式ドキュメントに明記された推奨パターンではなく、「Build commandは単なる
  1本のシェルコマンド文字列である」という確認済み仕様からの論理的な推測に過ぎないことを
  ユーザーからの追及を受けて訂正した。

**最終決定**: この不確実性を回避するため、デプロイはCloudflareダッシュボードのGit連携
（Workers Builds）を使わず、**GitHub Actionsから`wrangler deploy`を実行する**方式に変更した。
GitHub Actionsは1コマンドしか設定できないという制約が無く、複数ステップのシェルスクリプトを
自由に書けるため、この不確実性を受けない。

## doc_archへの反映（完了済み）

上記の意思決定に伴い、以下のファイルを更新済み。

- `doc_arch/overview.md`: 確定事項テーブルに「自前API層（Facade）のホスティング」の行を
  追加（フロントと同一のCloudflare Workersプロジェクトに同居）。全体構成図を、フロント配信と
  API層を1つのCloudflare Worker内の2機能として描く形に修正。
- `doc_arch/backend.md` §3: ホスティング形態を「同一Cloudflare Workerプロジェクトに同居、
  `run_worker_first`で`/api/*`のみ振り分け、CORS不要」に確定。`env.ASSETS.fetch()`
  フォールバックが不要である点を明記。ディレクトリ構成（`frontend/`＋`worker/`）を追記。
- `doc_arch/backend.md` §7: CORSの記述を「自前API層自体は同一オリジンのためCORS対応不要。
  Supabase StorageバケットのCORS設定のみ必要」に修正。
- `doc_arch/hosting-and-cicd.md`: デプロイ方式を「Cloudflareダッシュボードの Git 連携では
  なく、GitHub Actionsから`wrangler deploy`を実行する」に変更し、その理由（Workers Buildsの
  1コマンド制約、モノレポでのフロント/Worker分離構成を公式サポートすると明記した情報が
  見つからなかったこと）を明記。PR時（ビルド検証のみ）とmainブランチpush時（本番デプロイ）で
  ステップを分けた具体的なCI/CDフローを記載。

## 教訓（推測を事実のように話して訂正された事例）

このセッション中、確認不足の推測を確定情報のように説明してしまい、ユーザーからの追及を
受けて訂正した事例が2件あった。今後同種の調査をする際の注意点として記録する。

1. **Workers BuildsのBuild command回避策**（§11）: 「`cd ../frontend && pnpm install &&
   pnpm build`のようなコマンドで越境ビルドを回避できる」と、Build commandが単なる1本の
   シェルコマンド文字列であるという確認済み仕様からの論理的推測を、あたかも公式にサポート
   されている方法であるかのように説明してしまった。実際には「1つのWorkerが別ディレクトリの
   フロントエンドビルド成果物を使う」構成を公式が明記してサポートしているとは確認できず、
   最終的にはこの不確実性ごと回避するためGitHub Actions方式に切り替えた。
2. **SupabaseのPKCEフローのデフォルト値**（§8）: 「PKCEフローがデフォルトで有効」と誤った
   説明をした。後に`@supabase/auth-js`（`GoTrueClient`）の`DEFAULT_OPTIONS`のソースコードを
   直接確認し、`flowType`のデフォルトは`'pkce'`ではなく`'implicit'`であり、PKCEを使うには
   `createClient`で明示的に`flowType: 'pkce'`を指定する必要があると訂正した。

両ケースとも、一次情報（公式ドキュメントのコード例、ライブラリのソースコード）を直接確認する
前に一般論・推測で回答してしまった点が共通する。

## 今後の未決事項

- **Hono採用の正式決定はまだしていない**。ユーザーに見せた具体的なコードイメージはHonoを
  使う想定で書かれているが、これは「使ってよい選択肢」として提示されたのみであり、
  `itty-router`や素の`if`/`switch`分岐という代替も残っている
  （詳細は`docs_bevy_sample/20260802_hono-requirement-for-same-worker-frontend-backend.md`）。

## 関連ドキュメント

- `docs_bevy_sample/20260802_standalone-workers-backend-supabase-dynamodb-crud.md`
  （別Workerとしてバックエンドを新設する場合の実装調査。Hyperdrive・JWKS検証・DynamoDB接続）
- `docs_bevy_sample/20260802_hono-requirement-for-same-worker-frontend-backend.md`
  （同一Worker構成でのHono要否調査。未決定であることの根拠）
- `docs_bevy_sample/20260802_google-sso-feasibility-with-jwks-verification.md`
  （確定済みアーキテクチャ上でのGoogle SSO実現可否調査。PKCEデフォルト値の訂正込み）
- `docs_bevy_sample/20260731_auth0-supabase-third-party-auth.md`（Auth0が選択肢の一つに
  過ぎず、採用未決定であることの根拠）
- `docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md`（自前API層でフロントから
  Supabase/DynamoDBを隠蔽する方針の初出）
- `doc_arch/overview.md` / `doc_arch/backend.md` / `doc_arch/hosting-and-cicd.md`
  （本書の議論の結果として更新済みの確定アーキテクチャ）
