# Auth.js on Cloudflare Pages Functions + Supabase(素のPostgres) 技術調査

日付: 2026-07-30

## 0. 本調査の位置づけ

本調査は、`docs_bevy_sample/20260730_supabase-react-crud-sso-samples.md`（Supabase + React
CRUDサンプルの横断調査、Google/GitHub SSOログイン要件から派生）の続きとして行われたもの。

前回調査は「フロントから Supabase SDK を直接叩く」標準構成のCRUD/OAuth実装パターンを扱った。
今回はその一歩先、**認証そのものを Supabase Auth ではなく自前API層内の Auth.js に担当させ、
Supabase は素の Postgres としてのみ使う**という別案を検討している。

これは本リポジトリの `doc_arch/web-publish-and-ugc-architecture.md` §5.3・§8・§12 で
すでに提起されている以下の設計・未決事項と直結する。

- §5.3・§8: フロントは Supabase Auth JS SDK を例外的に使ってよいが、それ以外の Supabase 固有
  機能（PostgREST・RLS・Storage SDK）はフロントから直接叩かない、という Facade 方針。
- §12 未決事項: 「認証プロバイダも将来差し替える可能性を見込むか」

今回の調査は、この未決事項に対する一歩進んだ検討（認証自体を自前API層内の Auth.js に
担当させ、Supabase Auth への依存自体を切り離す案）にあたる。ただし
`web-publish-and-ugc-architecture.md` 自体の更新はユーザーが別途直接行うため、本ドキュメントは
調査結果の記録に留める。

## 1. Auth.js の Cloudflare Workers / Pages Functions 対応状況

**わかったこと**

- `@auth/core` はフレームワーク非依存・ランタイム非依存に設計されており、標準の `Request` を
  受けて `Response` を返す関数 `Auth(request, config)` として使える。Auth.js自身のコアロジック
  （署名検証、Cookie操作、OAuthフロー等）はWeb標準API（Web Crypto等）ベースで実装されており、
  「Auth.jsとそのコールバックだけを使い、他のNode依存ライブラリを混ぜなければどこでも動く」と
  公式が明言している。
- 一方で公式の Edge Compatibility ガイドは「edge runtimeは一般にTCPソケットを利用できない」
  という前提で書かれており、DBアダプタ（多くがTCP接続するPostgres/MySQLクライアントに依存）を
  edgeで使うのは非推奨、という書きぶりになっている。これは主にVercel Edge Runtimeのような
  「TCPソケット自体が存在しないランタイム」を念頭にした記述であり、**Cloudflare Workersは2022年
  以降 `connect()` という生TCPソケットAPIを提供しており、この前提に当てはまらない**。
- Cloudflare公式・Auth.js公式共に「Cloudflare Pages Functions + `@auth/core`（Next.js抜き）」の
  ズバリな統合テンプレートは見当たらなかった。ただし公式リポジトリに
  `nextauthjs/sveltekit-auth-cloudflare` という「SvelteKit Auth を Cloudflare Pages に
  デプロイする公式サンプル」が存在する（README詳細はWebFetchで完全取得できず、adapterや
  session戦略の記載は未確認）。Hono公式ドキュメントにはCloudflare Pages Functions向けの
  `handle(app)` パターンがあり、Auth.js専用ではないがHono経由で `@auth/core` をマウントする
  土台にはなる。
- 既知の詰まりどころ:
  - **`UntrustedHost` エラー**: Auth.jsはデフォルトでHostヘッダを信用しない。Cloudflare
    Pagesでは環境変数 `CF_PAGES` を検知して自動的に `trustHost: true` 相当の扱いにする対応が
    議論されている（`nextauthjs/next-auth` Discussion #12717）が、確実性のためコード側で
    明示的に `trustHost: true` を設定するのが実務上の回避策として報告されている。
  - `jose`パッケージ（Auth.jsのJWT処理に内部依存）がCloudflare上のNode互換層（unenv）の
    `crypto`ポリフィルと衝突して落ちる事例がGitHub Issueに報告されている。
  - bcryptはネイティブバインディング依存のため動かない。Credentials provider等でパスワード
    ハッシュが必要な場合は `bcryptjs`（純JS実装）を使う必要がある（今回はOAuthのみなので
    直接関係は薄い）。
  - バンドルサイズ上限: Cloudflare Workers/Pages Functionsはgzip後 **Freeプランで3MB、
    Paidプランで10MB**（圧縮前は両プラン64MBまで）。Auth.js本体は比較的軽量だが、`pg`ドライバや
    ORM（Drizzle/Kysely）、Prismaなどを足すと肥大化しやすく、Prismaは特にWorkers環境で
    バンドルサイズ・Node依存の両面で相性が悪いことで知られる。

**出典URL**:
- https://authjs.dev/guides/edge-compatibility
- https://authjs.dev/reference/core
- https://github.com/nextauthjs/sveltekit-auth-cloudflare
- https://hono.dev/docs/getting-started/cloudflare-pages
- https://github.com/nextauthjs/next-auth/discussions/12717
- https://github.com/nextauthjs/next-auth/issues/8532
- https://github.com/nextauthjs/next-auth/discussions/8547
- https://developers.cloudflare.com/workers/platform/limits/

**未確認・要検証な点**: `sveltekit-auth-cloudflare` の実際のREADME本文（採用アダプタ・
セッション戦略・wrangler設定）は未確認、クローンして中身を見る必要あり。Cloudflare Pages
Functions固有の制約（コールドスタート特性、`_middleware.ts`とAuth.jsのCookie書き込みタイミング等）
は一次情報が見つからず未検証、実機検証が必要。

## 2. セッション管理の方式

**わかったこと**

- Auth.jsは **JWT戦略**（デフォルト、DB不要でCookieに署名付きトークンを保持）と
  **データベース戦略**（`sessions`テーブルにセッションを保存し、Cookieにはランダムな
  セッショントークンのみ持たせる）の2つをサポート。
- Cloudflare Workers/Pages Functionsのようなステートレスで短命な実行環境では、一般論として
  「ステートレスなJWTの方が相性が良い」（DBラウンドトリップ不要、リージョン分散環境での
  毎リクエストDB接続コストを避けられる）。データベース戦略はセッション即時失効・強制ログアウト
  等の制御力と引き換えに、毎リクエストDBアクセスが発生する。ロール変更を即時反映したい場合は
  JWT戦略だと再ログインが必要になるトレードオフがある。
- データベースセッションを使う場合の接続経路: Cloudflare Workers/Pages FunctionsからPostgres
  への接続は、2022年公開の `connect()`（`cloudflare:sockets`）という生TCPソケットAPIにより
  理論上可能。`node-postgres`(`pg`)は `pg-cloudflare` という内部シムを介してこの`connect()`を
  使うようになっており、Wrangler設定で `nodejs_compat` フラグ（互換日付2024-09-23以降）を
  有効にすれば `pg` パッケージがCloudflare Workers上で動作する、と複数の一次情報が明言している。
- ただしCloudflareは **Hyperdrive** の利用を強く推奨している。Hyperdriveは「Workersからの
  Postgres接続をグローバルにコネクションプーリング・キャッシュするCloudflareのマネージド
  プロキシ」で、Workerの短命な実行モデルとPostgresの持続的接続モデルのミスマッチを吸収する。
- **重要**: Cloudflare Pages Functions は Workers ランタイム上で動作するため、Hyperdrive
  バインディングも `wrangler.toml` に `[[hyperdrive]]` セクションを書くことでPages
  Functionsから利用可能（Node.js互換モード必須）。ただしCI/CD経由のデプロイ（GitHub Actions等）
  でHyperdrive設定が絡む既知の不具合報告があり、ローカル `wrangler pages deploy` では動くが
  CI連携で詰まる事例がある。
- Auth.js自身の公式ガイド（edge-compatibility）は「edgeは一般にTCPソケット不可」という
  一般論の記述に留まり、Cloudflare Workersの `connect()`/Hyperdriveという例外については
  言及していない点に注意（Auth.js公式ドキュメントの想定は主にVercel Edge Runtime等）。

**出典URL**:
- https://authjs.dev/concepts/session-strategies
- https://blog.cloudflare.com/workers-tcp-socket-api-connect-databases/
- https://github.com/brianc/node-postgres/tree/master/packages/pg-cloudflare
- https://developers.cloudflare.com/workers/tutorials/postgres/
- https://developers.cloudflare.com/hyperdrive/examples/connect-to-postgres/
- https://developers.cloudflare.com/hyperdrive/examples/connect-to-postgres/postgres-database-providers/supabase/
- https://developers.cloudflare.com/pages/functions/bindings/
- https://github.com/cloudflare/workers-sdk/issues/5525（Pages+Hyperdriveの既知不具合）

**未確認・要検証な点**: `pg`単体（Hyperdriveなし、`connect()`直叩き）でCloudflare Pages
Functionsから安定して本番運用できるか（コネクション数上限・レイテンシ）は一次情報の
ベンチマークが見つからず未検証。`@auth/pg-adapter`のクエリパターンとHyperdriveのキャッシュ・
プリペアドステートメント方針との整合性も未検証。

## 3. Auth.js のアダプタ

**わかったこと**

- Postgres向けの公式アダプタは複数存在: `@auth/pg-adapter`（生の`pg`の`Pool`を渡すだけの
  薄いアダプタ）、`@auth/drizzle-adapter`、`@auth/kysely-adapter`、`@auth/prisma-adapter`、
  さらにSupabase専用の `@auth/supabase-adapter`、Neon専用の `@auth/neon-adapter` もある。
- `@auth/pg-adapter` は `npm install next-auth @auth/pg-adapter pg` で導入し、`pg.Pool`
  インスタンスを渡すだけでよい。**SupabaseのSDK（`@supabase/supabase-js`やPostgREST）は
  一切経由せず、素の接続文字列（`postgres://user:pass@host:port/db`）だけで動く**ため、
  「Supabase固有の仕組みに依存しない」という元の設計方針とも整合する。
- Auth.jsが要求する標準スキーマは4テーブル:
  - `users`（`id`, `email`, `emailVerified`, `name`, `image`）
  - `accounts`（`userId`, `provider`, `providerAccountId`, `access_token`, `refresh_token`,
    `expires_at`等、OAuthのトークン保存用）
  - `sessions`（`sessionToken`, `userId`, `expires`。データベースセッション戦略時のみ
    実質必要）
  - `verification_tokens`（`identifier`, `token`, `expires`。マジックリンク等パスワードレス
    認証用、OAuthのみなら未使用）
- 既存のロール管理テーブルとの共存は一般的に「`users`テーブルに`role`カラムを追加する」か
  「別テーブル（例: `user_roles`）を`users.id`に外部キーで紐付ける」のどちらか。Auth.js
  公式のRBACガイドはPrisma例で `User`モデルに`role`カラムを直接足すパターンを示している。
  既存の「ロール管理用Postgresテーブル」がすでにあるなら、Auth.js標準の`users`テーブルとは
  別に維持しつつ、`session`/`jwt`コールバック内で手動JOINしてロールを取得する設計も可能
  （アダプタの標準スキーマを汚さずに済む）。

**出典URL**:
- https://authjs.dev/reference/core/adapters
- https://authjs.dev/reference/pg-adapter
- https://authjs.dev/reference/drizzle-adapter
- https://authjs.dev/getting-started/adapters/kysely
- https://authjs.dev/guides/role-based-access-control

**未確認・要検証な点**: `verification_tokens`テーブルはOAuthのみの運用なら未使用になる
可能性が高いが、アダプタの初期化が当該テーブルの存在を必須とするかは未確認。

## 4. Postgres接続方式とSupavisorの注意点

**わかったこと**

- CloudflareのHyperdrive公式ドキュメントは、Supabaseと組み合わせる際「**Direct connection
  文字列を使い、Supavisor（プールド接続）は使うな**」と明記している。理由は「Hyperdrive自身が
  グローバルにコネクションプーリングを行うため、Supavisorと二重にプーリングすると
  非効率／不整合が起きる」ため。
- Supabase側の一次情報でも、Supavisorの**transaction mode（ポート6543）はprepared
  statementを正式にはサポートしない**（近年「named prepared statement」の限定サポートが
  追加されたが、`pg`のデフォルト動作である無名prepared statementの多用にはまだ制約がある）
  ことが明言されている。Prisma等では接続文字列に`pgbouncer=true`を付与してprepared
  statementを無効化する回避策が案内されている。**session mode**ならprepared statementも
  使える。
- まとめると、Cloudflare Hyperdrive経由でSupabaseに繋ぐ場合は「Supabaseの直接接続
  （Direct connection、通常ポート5432）」を使うのが公式推奨であり、Supavisorを二重に
  挟まない設計が正しい。

**出典URL**:
- https://developers.cloudflare.com/hyperdrive/examples/connect-to-postgres/postgres-database-providers/supabase/
- https://supabase.com/docs/guides/troubleshooting/disabling-prepared-statements-qL8lEL
- https://supabase.com/docs/guides/troubleshooting/supavisor-faq-YyP5tI
- https://supabase.com/blog/supavisor-postgres-connection-pooler

**未確認・要検証な点**: SupabaseのDirect connectionはIPv6アドレスのみ提供される場合があり、
Cloudflare側（Hyperdrive/WorkersのTCPソケット）からのIPv6到達性、あるいはSupabase側の
IPv4アドオン要否は未確認。Direct connectionは無料プランでは同時接続数が少ないため、
想定同時接続数に応じてプラン・Compute Add-onの要否確認が必要。

## 5. Google/GitHub OAuth プロバイダ設定

**わかったこと**

- Auth.jsの `GoogleProvider` / `GitHubProvider` はどちらも `clientId` / `clientSecret` を
  渡すだけの標準的な設定。コールバックURLの形式は共通で `{origin}{basePath}/callback/{provider}`
  （例: 本番で `https://yourdomain.com/api/auth/callback/google`、GitHubも同様に
  `.../callback/github`）。
- GitHub固有の注意点: **GitHubのOAuth Appは1アプリにつきコールバックURLを1つしか登録
  できない**。開発用と本番用でURLが異なる場合、GitHub側でアプリを2つ（dev用・prod用）作る
  必要がある（Googleは複数の「承認済みリダイレクトURI」を1つのクライアントに登録できるため
  この制約はない）。
- 前回調査した「Supabase Social Login」方式との違い: Supabase Authを使う場合はコールバック
  URLがSupabaseのプロジェクトドメイン（`https://<project-ref>.supabase.co/auth/v1/callback`）
  に固定され、OAuthのやり取り自体はSupabase側で完結する。Auth.js自前運用の場合は、
  コールバックURLが**自前のCloudflare Pages Functionsのドメイン**になり、OAuthのトークン
  交換・プロフィール取得処理もすべて自前API層内（Auth.jsのコード）で実行される。つまり
  「OAuthプロバイダに対して名乗り出るホスト」がSupabaseから自社ドメインに変わる、という点が
  設計上の一番の違い。

**出典URL**:
- https://authjs.dev/guides/configuring-github
- https://authjs.dev/reference/core/providers/google
- https://authjs.dev/getting-started/deployment

## 6. ロール管理の実装パターン

**わかったこと**

- Auth.js公式のRole Based Access Controlガイドが示す標準パターン:
  1. OAuthプロバイダの`profile()`コールバックで、外部プロフィール情報からロールを決定する
     責任はアプリ側にある（デフォルトは`"user"`など）。
  2. **JWT戦略**の場合: `jwt`コールバックで`token.role = user.role`のようにトークンへ
     ロールを載せ、Cookieに署名して保存する。
  3. **データベース戦略**の場合: `profile()`の戻り値がそのまま`users`テーブルのレコード
     作成に使われ、`role`カラムに保存される。
  4. どちらの戦略でも、クライアントに公開したい場合は`session`コールバックで
     `session.user.role = token.role`（JWT）または`session.user.role = user.role`（DB）の
     ように明示的に詰め替える必要がある（デフォルトでは伝播しない）。
- トレードオフ: JWT戦略はロール変更が即座に反映されず、ユーザーの再ログイン（または
  トークン再発行）が必要になる。DB戦略は`session`コールバックの中で毎回DBを見に行く実装に
  すれば即時反映できるが、リクエストごとにDB問い合わせが発生する。
- 保存場所は「Auth.jsの`users`テーブルにロールカラムを追加する」のが最も単純だが、既存の
  別ロール管理テーブルがある場合は、`session`/`jwt`コールバックの中で`users.id`をキーに
  手動でそのテーブルをJOIN/クエリして取得し、トークン/セッションに詰める実装が現実的。

**出典URL**: https://authjs.dev/guides/role-based-access-control

## 7. 代替案（Auth.jsで詰まった場合の逃げ道）

**わかったこと**

- **Lucia**: 「シンプルで軽量、Cloudflare Workersを含む任意のランタイムで動く」として
  人気だったセッション管理ライブラリだが、開発チームが**Luciaを非推奨とし、より小さな部品
  である `oslo`（暗号・パスワードハッシュ・Cookie等のユーティリティ集）と `Arctic`
  （OAuth 2.0クライアントライブラリ、50以上のプロバイダに対応）に分割・移管する方針**を
  表明している（2024年発表）。したがって現時点で新規に選ぶなら「Lucia」そのものよりも
  「Arctic（OAuthクライアント）＋自前のセッション管理（Cookie＋DB or JWT）」という構成が
  実質的な後継。
- Arctic単体は「OAuthの認可コードフロー・トークン交換だけ」を薄く提供するライブラリで、
  セッション管理・ユーザーDB・ロール管理は完全に自前実装になる（Auth.jsのような
  「フルスタックの認証フレームワーク」ではない）。Cloudflare Workers上での動作実績も
  報告あり（Web標準API・`fetch`ベースのため相性が良い）。
- Cloudflareエコシステム内には`@auth0/auth0-hono`（Auth0連携）や`Better Auth`（Cloudflare
  Workers bindings対応を謳う新興ライブラリ）といった選択肢も存在するが、それぞれ別の
  トレードオフ（Auth0は外部IDaaS依存、Better AuthはAuth.js同様の新しめのエコシステム依存）を
  伴う。

**出典URL**:
- https://github.com/lucia-auth/lucia
- https://github.com/lucia-auth/lucia/discussions/1707
- https://lucia-auth.com/
- https://hono.dev/examples/better-auth-on-cloudflare
- https://auth0.com/blog/adding-auth0-hono-cloudflare-workers-guide/

## 8. 結論: Cloudflare Pages Functions + Auth.js + Supabase(素のPostgres) 構成は現実的か

**結論**: **技術的には実現可能。ただし「公式のワンストップ・テンプレート」は存在せず、
複数の一次情報を組み合わせて自前で配線する必要がある「上級者向け構成」**という位置づけに
なる。

根拠の要点:

- `@auth/core`自体はWeb標準API・ランタイム非依存で書かれており、Cloudflare Pages
  Functions（＝Workersランタイム）上で動く土台はある。
- Auth.js公式のedge-compatibilityガイドが警告する「edgeはTCPソケット不可」という制約は、
  **Cloudflare Workersには実質当てはまらない**（`connect()` TCPソケットAPI +
  `pg-cloudflare`シム + Hyperdriveにより、`pg`パッケージ経由でPostgresに接続できる）。
  ここはAuth.js公式ドキュメントの記述とCloudflareの実際の能力にギャップがあり、混同すると
  「Cloudflareでは無理」と誤解しやすいポイント。
- Postgresアダプタ（`@auth/pg-adapter`）はSupabase SDKを経由せず素の接続文字列で動くため、
  「BaaS固有機能に依存しない」という元の設計方針とも整合する。
- OAuthプロバイダ設定・ロール管理コールバックは通常のAuth.jsの使い方の範囲内で実現可能。

実装する場合の注意点・推奨手順:

1. **セッション戦略はまずJWTを既定にする。** データベースセッションは「即時失効」等の
   強い要件がある場合のみ検討し、その場合もHyperdrive必須で設計する。
2. **Postgres接続は必ずHyperdrive経由にし、Supabase側は「Direct connection」を使う
   （Supavisorを二重に挟まない）。** これはCloudflare公式が名指しで推奨している構成。
3. **`nodejs_compat`フラグ（互換日付2024-09-23以降）を`wrangler.toml`に設定**し、Pages
   Functions側でもHyperdriveバインディング（`[[hyperdrive]]`）を有効化する。CI/CD
   （GitHub Actions等）経由のデプロイでは既知の不具合報告があるため、まずローカル
   `wrangler pages deploy`で動作確認してからCI化する。
4. **`trustHost: true`を明示的にAuth.js設定に書く**（環境変数の自動検知に頼らずコード側で
   固定し、`UntrustedHost`エラーを避ける）。
5. **ロール管理テーブルは既存のPostgresの別テーブルのままでよく**、`session`/`jwt`
   コールバック内で`users.id`をキーに手動クエリして詰め込む設計にすれば、Auth.js標準
   スキーマ（`users`/`accounts`/`sessions`/`verification_tokens`）を汚さずに済む。
6. **verification_tokensテーブルはOAuthのみの運用でも作成が必要になる可能性が高い**ため、
   事前にAuth.js側のアダプタ初期化コードで実際に必須か検証する。
7. **早い段階でPoC（Google/GitHubログイン→Postgresへのユーザー作成→ロール読み出しまでの
   最小構成）をCloudflare Pages Functions上に実際にデプロイして動作確認する。** ドキュメント
   上は動きそうでも、Cloudflare特有のバンドルサイズ制限・CI連携の不具合・IPv6到達性など
   「実機でしか出ない詰まり」が複数報告されているため、設計だけで進めずに早期に実証すべき。
8. **もしPoCでAuth.js特有の制約（バンドルサイズ超過、アダプタのNode依存衝突等）に当たった
   場合の代替**として、Arctic（OAuthクライアント部分のみ）＋自前セッション管理への切り替えを
   想定しておく。Luciaは非推奨化されているため新規採用は避ける。

## 9. 本リポジトリへの示唆（再掲・参照用）

本調査は `doc_arch/web-publish-and-ugc-architecture.md` §12 の未決事項「認証プロバイダも
将来差し替える可能性を見込むか」に対する一案の技術的裏付けとして行ったもの。同ドキュメントの
更新自体は別途ユーザーが直接行う想定であり、本ドキュメントでは以下の対応関係を記録するに
留める。

- 現行の§5.3方針（フロントはSupabase Auth JS SDKを例外的に許可、それ以外のSupabase固有機能は
  自前API層に隠蔽）を維持する場合 → 本調査の内容は直接は適用されない（Supabase Authを
  そのまま使い続けるため）。
- 認証自体をSupabase Authから切り離し、自前API層内のAuth.jsに担当させる場合 →
  本調査の§1〜8がそのまま設計・実装の出発点になる。Supabase Postgresは素のPostgresとして
  §3・§4の接続方式（Hyperdrive + Direct connection）でアクセスすることになる。
