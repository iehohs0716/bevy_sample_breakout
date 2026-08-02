# バックエンド設計

## この文書について

全体像は [overview.md](./overview.md) を参照。フロントエンドからの利用のされ方は
[frontend.md](./frontend.md) を、ローカルでの起動方法は [deploy.md](./deploy.md) を、
本番ホスティングは [hosting-and-cicd.md](./hosting-and-cicd.md) を参照。

---

## 1. 採用: Supabase（Auth・ユーザー情報）＋ DynamoDB（ゲームデータ）のハイブリッド

バックエンドは 2 系統に分かれる。

- **Supabase**: Auth（ログイン・JWT発行）とユーザー情報（Postgres）を担当。Docker Compose
  でのローカル開発と相性が良く（Supabase CLI がそのまま Docker ベースのローカルスタックを
  提供する。詳細は [deploy.md](./deploy.md)）、ユーザー関連のリレーショナルなクエリも
  Postgres の SQL でそのまま組める。
- **DynamoDB**: ゲームシナリオ／ゲーム／ブロック配置といった「ゲームデータ」を担当（§2）。
  こちらは KVS としての読み書き（キー＝シナリオ ID で丸ごと取得）が主なアクセスパターンであり、
  リレーショナルな検索よりも低レイテンシな単純取得に適性がある。
- **Supabase Storage**: 画像バイナリの実体（背景・ブロック画像）を担当。ユーザー情報・ゲームデータ
  どちらからも URL 参照のみ持つ（§5.3）。

この構成は、以前検討していた「Supabase 単体（Postgres に一本化）」案と「Supabase＋DynamoDB の
ハイブリッド」案のうち、**後者を採用**したことを意味する。

## 2. ゲームデータの保存先: DynamoDB

### 2.1 採用理由

ゲームシナリオ（複数のゲームを束ねた再生単位）は、以下の性質を持つ。

- **URL 単位でアクセスされる**＝シナリオ ID をキーに「丸ごと 1 件」取得するアクセスパターンが
  ほぼすべてであり、JOIN や複雑な検索条件を必要としない。
- **アグリゲート内の整合性**（シナリオに属するゲームの順序・各ゲームの画像参照とブロック配置）
  さえ保たれればよく、シナリオをまたいだリレーショナルな整合性は不要。

この性質は DynamoDB のようなキーバリュー型ストアに向いている。ユーザー情報（一覧・検索・
将来のいいね機能など、リレーショナルなクエリが今後増える見込みが高いもの）とは要件が異なるため、
Supabase の Postgres に無理に一本化せず、ゲームデータだけ DynamoDB に分離する。

### 2.2 テーブル設計

- **パーティションキー**: `scenarioId`（シナリオ単位で 1 アイテム）
- シナリオに属する全ゲーム（`games`）・各ゲームの画像参照・ブロック配置は、**同じアイテム内に
  ネストした構造で丸ごと格納する**（DynamoDB の Map / List 型を利用）。これにより「シナリオを
  プレイする」という主要ユースケースが 1 回の `GetItem` で完結する。
- スキーマの詳細は §5.1 のデータモデルを参照。

### 2.3 ローカル開発

`amazon/dynamodb-local`（AWS 公式 Docker イメージ）を Supabase CLI のスタックと並行して
Docker Compose に追加する（詳細は [deploy.md](./deploy.md)）。

## 3. ポータビリティ確保の方針（Facade で Supabase を隠蔽する）

**課題**: Supabase の「素早く作れる」という強みは、フロントから `supabase-js` 経由で
PostgREST を直接叩き、認可を RLS（Row Level Security）ポリシー内の `auth.uid()` に
任せる、という使い方をして初めて最大化される。しかしこれをそのまま採用すると、

- レベル一覧・詳細の取得 URL が PostgREST 固有のクエリ規約（`?visibility=eq.public` 等）
- 投稿の可否判定が Postgres の RLS ポリシー（`auth.uid()` は Supabase Auth が
  Postgres セッションに注入する Supabase 固有の仕組み）
- 画像の出し入れが `supabase-js` の Storage SDK の関数シグネチャ

という **Supabase 固有の規約にフロントのコードが直接依存**する。Postgres の「データ」自体は
`pg_dump` で Neon 等へ移せても、これらの規約は Neon 単体（素の Postgres）には存在しないため、
移行時にフロントとアクセス層をまるごと作り直すことになる。「データベースだけ差し替えれば済む」
状態にはならない。

**方針**: フロントエンドは Supabase の SDK・PostgREST・Storage SDK を **直接叩かない**
（[frontend.md](./frontend.md)）。代わりに、アプリ自身が所有する薄い API 層（Cloudflare
Workers）を挟み、**フロントはこの自前 API 層としか通信しない**。Supabase 固有の呼び出し方は
すべてこの API 層の内部実装に閉じ込める。実装としては一般的な **Facade パターン**（＋ Storage
部分は Adapter パターン）そのものであり、目新しい仕組みを導入するわけではない。

**ホスティング形態**: この自前 API 層は、フロント（React + WASM の静的アセット）と**同一の
Cloudflare Workers プロジェクト**に同居させる。Workers Static Assets の `run_worker_first`
（例: `["/api/*"]`）で `/api/*` 宛のリクエストのみ Worker 側のロジックに振り分ける。
`run_worker_first` に一致しないリクエストは Worker 本体を経由せず Workers Static Assets が
直接処理するため、Worker のコード側に `env.ASSETS.fetch()` によるフォールバック実装は不要。
フロントと自前 API 層は同一オリジンになるため CORS 対応も不要（§7）。

**ディレクトリ構成**: ソースコードは `frontend/`（React + WASM。`src/` 配下は Feature-Sliced
Design。詳細は [frontend.md](./frontend.md)）と `worker/`（自前 API 層本体）の 2 つに分ける。
FSD はフロントエンド専用の方法論であり `worker/` には適用されない。

```
frontend/            # Vite + React（src/ 配下は FSD）
├── src/
└── dist/            # `vite build` の成果物
worker/              # Cloudflare Worker 本体（自前 API 層）
├── src/
│   └── index.ts
└── wrangler.jsonc    # main: "./src/index.ts", assets.directory: "../frontend/dist"
```

`worker/wrangler.jsonc` の `assets.directory` が `frontend/dist` を相対パスで参照する形にし、
`worker/` から `wrangler deploy` を実行すると両者が 1 つの Cloudflare Worker としてデプロイされる。
デプロイ手順・CI/CD 構成は [hosting-and-cicd.md](./hosting-and-cicd.md) を参照。

Worker 側ルーティングの実装（Hono 等の採用可否を含む）は自由度が高く、
`docs_bevy_sample/20260802_hono-requirement-for-same-worker-frontend-backend.md` を参照。
Supabase/DynamoDB 接続方式の実装調査は
`docs_bevy_sample/20260802_standalone-workers-backend-supabase-dynamodb-crud.md`
（CORS の節 §2 を除き、Hyperdrive・JWT 検証・DynamoDB 接続の内容はそのまま参考にできる）。

| 要素 | Supabase 標準の使い方 | ロックインの度合い | ポータブルにする方法 |
|---|---|---|---|
| データ本体 | Postgres テーブル | 低（標準 SQL） | Supabase 固有の拡張機能には依存しない標準 SQL のみで組む |
| データアクセス方式 | PostgREST 直叩き + RLS | **高**（Supabase 固有の URL 規約・`auth.uid()`） | API 層が標準的な SQL ドライバでクエリを発行し、認可判定も API 層側のコードで行う（RLS に丸投げしない） |
| 画像ストレージ | `supabase-js` Storage SDK 直叩き | 中〜高 | API 層がアップロード／配信を仲介するアダプタを持ち、フロントは自前 API のエンドポイントしか知らない |
| 認証 | Supabase Auth が発行する JWT | 中（JWT 形式自体は標準的だが発行元は Supabase 固有） | API 層は標準的な JWT 検証のみ行い、発行元（Issuer）を差し替え可能な作りにする |

**トレードオフの明示**: この方針は、Supabase の「サーバーレスで自前バックエンド不要」という
最大のメリットを一部手放し、API 層のコードを自前で書く分、開発速度は PostgREST 直叩きより
落ちる。しかし「将来 Neon 等へ移行する可能性がある」というポータビリティ要件の優先度が
開発速度より高いため、本書ではこちらを採用する。以降 §6（API設計）・[frontend.md](./frontend.md)
はこの方針を前提に記述する。

**注記（DynamoDB との関係）**: 上表はユーザー情報側（Supabase/Postgres）のポータビリティ
（＝将来 Neon 等へ移行できるようにする）を主目的とした整理である。ゲームデータ側の DynamoDB
（§2）は、ポータビリティではなく KVS としての運用適性を理由に採用しており、AWS への依存は
許容する判断である。ただし DynamoDB へのアクセスも同じ Facade（自前 API 層）の内部に閉じ込め、
フロントからは直接叩かせない点は同じ（[overview.md](./overview.md) の確定事項表）。

## 4. 認証層のさらなるポータビリティ強化案（検討中・未確定）: Auth.js

§3 の表にある通り、認証は現状「JWT 発行元が Supabase 固有」という中程度のロックインが
残っている（[frontend.md](./frontend.md) で述べる通り、フロントが例外的に Supabase Auth の
JS SDK を直接使うことを許容しているのもこのため）。これをさらに解消する案として、**OAuth の
やり取りそのものを Supabase Auth ではなく自前 API 層（Cloudflare Workers）内で動かす
Auth.js に担当させ、Supabase は §2 と同じく素の Postgres（ロール管理テーブル含む）としてのみ
使う**という構成を検討中。技術調査の詳細は
`docs_bevy_sample/20260730_authjs-oauth-on-cloudflare-pages-functions.md`
（関連: `docs_bevy_sample/20260730_supabase-react-crud-sso-samples.md`）にまとめてある。

調査で確認できた要点:

- `@auth/core` はランタイム非依存（Web標準API ベース）で、Cloudflare Workers ランタイム上でも
  動作する土台がある。Auth.js 公式の Edge Compatibility ガイドが警告する「edge は TCP ソケット
  不可」という制約は、Cloudflare Workers には実質当てはまらない（`connect()` TCP ソケット API
  ＋ Hyperdrive 経由で Postgres に接続できるため）。
- Postgres 用アダプタ（`@auth/pg-adapter` 等）は Supabase の SDK・PostgREST を一切経由せず、
  素の接続文字列だけで動く。これは §3 の方針（Supabase 固有機能に直接依存しない）と整合する。
- ただし「公式のワンストップ・テンプレート」は存在せず、Hyperdrive の設定（Supabase 側は
  Direct connection を使い Supavisor と二重に挟まない等）、`nodejs_compat` フラグ、
  `trustHost` の明示設定などを自前で配線する必要がある「上級者向け構成」であり、実装前に
  Cloudflare Workers 上での PoC（Google/GitHub ログイン→Postgres へのユーザー作成→
  ロール読み出し）が必須。

**採用した場合に本書へ与える影響（採用を決めた時点で反映すること）**:

- §3 の表の「認証」行は「ロックインの度合い: 低」に変わる（JWT 発行元も自前 API 層になるため）。
- [frontend.md](./frontend.md) の「認証フローは例外的に Supabase Auth の JS SDK を使ってよい」
  という記述は撤回し、「ログインも自前 API 層のエンドポイント（Auth.js）を経由する」に置き換える。
- [overview.md](./overview.md) の「認証プロバイダを将来差し替える可能性を見込むか」は
  本節の採用可否と一体で決着する。

現時点ではまだ **PoC 未実施・採用は未確定** であり、[overview.md](./overview.md) の確定事項表・
[frontend.md](./frontend.md) の記述は変更していない。

---

## 5. データモデル

### 5.1 ゲームシナリオ定義スキーマ（案）

クラス図: [diagrams/game-scenario-class-diagram.drawio](./diagrams/game-scenario-class-diagram.drawio)
（ゲームシナリオ 1 ── 1..\* ゲーム ── 0..\*/2 画像、ゲーム ── 1 ブロック配置）。

正式なスキーマ定義（JSON Schema）: [schemas/game-scenario.schema.json](./schemas/game-scenario.schema.json)。
DynamoDB 自体はスキーマレス（強制されるのはパーティションキーの型のみ）だが、アイテム本体の
契約を OpenAPI 的に一箇所で宣言しておくためのドキュメントとして用意した。API 層でのバリデーション
実装（Zod 等）に落とし込む際も、この JSON Schema を正とする。

この UML を DynamoDB の 1 アイテムとして表現すると以下のようになる。

```jsonc
// DynamoDB: パーティションキー = scenarioId
{
  "scenarioId": "scn_01HXYZ...",     // ULID/UUID。URL からのアクセスキーにもなる
  "title": "初心者向けシナリオ",
  "authorId": "usr_...",             // Supabase 側ユーザーID（匿名投稿を許すなら nullable）
  "visibility": "public",            // "public" | "unlisted"
  "createdAt": "2026-07-30T00:00:00Z",
  "scenarioParameters": {
    // このリストの並び順がそのまま「ゲームの順番」を定義する
    "games": [
      {
        "gameId": "game_01...",
        "background": {
          "imageUrl": "https://.../scenarios/scn_.../game_01/background.png",
          "mime": "image/png"
        },
        "brickImage": {
          "imageUrl": "https://.../scenarios/scn_.../game_01/bricks.png",
          "mime": "image/png"
        },
        "cellSize": { "width": 50, "height": 30 },
        "brickPlacement": {
          "bricks": [{ "x": -200, "y": 100 }, { "x": -150, "y": 100 }, "..."]
        }
      }
      // ,{ "gameId": "game_02...", ... } ...
    ]
  },
  "stats": { "playCount": 0, "likeCount": 0 }
}
```

- 「画像」エンティティ（UML上、ゲーム 1 件につき 2 件＝背景・ブロック画像）は `background` /
  `brickImage` として、実体（画像バイナリ）を持たず **URL 参照のみ** を持つ。これは
  `injection.rs` の現行仕様（`backgroundBytes: Uint8Array` によるバイト列直接受け渡し）との
  差分であり、フロント側でシナリオ取得後に画像を `fetch` してバイト列化し、既存の
  `window.__BREAKOUT_CONFIG__` 形式に変換してから Bevy に渡す（変換はフロントの責務、
  game_engine は無改修。詳細は [frontend.md](./frontend.md)）。
- 「ブロック配置」エンティティは `brickPlacement` として各ゲームに 1 件、ネストして保持する。
- ゲーム自体を独立したキーで個別取得する要件（例: ゲーム単体の使い回し・シナリオ間共有）が
  将来出てきた場合は、`games` を別アイテム（`gameId` をパーティションキーとする別エンティティ）
  に切り出す設計に変更できる。MVP では「シナリオを丸ごと 1 回で取得してプレイする」が主要な
  アクセスパターンのため、ネスト構造で十分とする。

### 5.2 保存先の割り当て

| データ | 保存先 | 理由 |
|---|---|---|
| `background.imageUrl` / `brickImage.imageUrl` が指す実体（画像バイナリ） | Supabase Storage | 大容量バイナリはオブジェクトストレージが適する |
| ゲームシナリオ／ゲーム／ブロック配置（上記 JSON 全体） | DynamoDB（§2 参照） | シナリオ ID キーでの丸ごと取得が主なアクセスパターンの KVS データ |
| ユーザー情報・認証情報・セッション | Supabase（Postgres + Auth） | リレーショナルなクエリ（一覧・検索等）が今後増える見込み。自前実装しない |

### 5.3 画像配信と CORS

既存の `docs_bevy_sample/20260711_external-image-cors-and-formats.md` の教訓（`curl` で
200 が返ってもブラウザの `fetch()` は ACAO ヘッダが無いとブロックする）は、Supabase Storage
採用後も同様に当てはまる。**別途 S3 等の画像ホスティングを新設する必要はなく、Supabase
Storage 1 箇所に画像を集約してよい**が、以下は明示的に対応する。

- レベル画像を置くバケットは **public** にする（読み取りは未認証で可）。public バケット自体は
  追加の CORS 設定なしでも GET 自体は通るが、ブラウザからの `fetch()` を確実に通すため
  バケットの CORS 設定（`allowedOrigins`）に本番ドメイン（Cloudflare Workers の URL）と
  ローカル開発オリジン（`http://localhost:5173` 等）を明示登録する。
- アップロードは `supabase-js` の Storage SDK 経由に統一する（presigned URL を自前で組み立てる
  方式は CORS エラーの報告例が複数あり、避ける）。
- 「Supabase Storage の設定上は通っているはずが実ブラウザでは弾かれる」という既存の罠がある以上、
  **本番相当のドメイン・オリジンで Playwright 実機確認するまでは CORS 対応完了とみなさない**
  （`CLAUDE.md` の「実ブラウザで検証する」方針をそのまま踏襲）。

---

## 6. API設計（自前 API 層経由）

§3 の方針に基づき、フロントは PostgREST や Storage SDK を直接叩かない。Cloudflare Workers
上に置く自前 API 層が、フロントから見た唯一の窓口になる（[hosting-and-cicd.md](./hosting-and-cicd.md)）。

| 操作 | フロントから見た契約（自前 API） |
|---|---|
| レベル一覧取得（公開分のみ） | `GET /api/levels` |
| レベル詳細取得 | `GET /api/levels/:id` |
| レベル投稿 | `POST /api/levels`（要認証） |
| 画像アップロード | レベル投稿に含める（別エンドポイントに分けても可） |

フロントが知っているのはこの契約だけで、内部で DynamoDB にどんな問い合わせをしているか、
画像をどこに保存しているかは API 層の実装詳細として隠蔽される（§3）。これにより、
ゲームデータの保存先や画像ストレージの実体を差し替える作業は API 層の内部だけに閉じ込められる。

具体的な接続方式（Cloudflare Workers から DynamoDB・Supabase Postgres への接続方法等）
は実装着手時に個別検証する（[overview.md](./overview.md) 未決事項）。

---

## 7. セキュリティ・認可

- 画像アップロード: サイズ上限・MIME ホワイトリスト（png/jpeg/webp）をクライアント＋
  サーバー（Storage ポリシー / Edge Function）の両方で検証
- ブロック座標: 異常値（アリーナ範囲外・極端な個数）をサーバー側でも検証し、Bevy 側に
  不正な大量データを渡さない
- 認可判定は自前 API 層のコードで行う（§3）。`visibility = 'public'` のみ未認証で
  読み取り可、書き込みは投稿者本人のみ許可、というルールを API 層に実装する。
  ユーザー情報側（Supabase/Postgres）については RLS を「API 層のバグに対する保険」として
  追加で有効化してもよいが、それ単体を認可の主体にはしない（RLS に依存すると Neon 等への
  移行時に丸ごと作り直しになるため）。ゲームデータ側（DynamoDB）には RLS 相当の仕組みが
  存在しないため、認可は完全に API 層のコードのみで担保する
- CORS: 自前 API 層はフロントと同一オリジン（同じ Cloudflare Worker）になるため、API 呼び出し
  自体には CORS 対応は不要。ただし Supabase Storage バケットの `allowedOrigins` は本番ドメイン・
  開発オリジンに限定登録する（詳細は §5.3）。ワイルドカード許可はしない
- モデレーション: MVP では通報機能なし。公開前提での運用リスクとして明記し、
  将来拡張（[requirements.md](./requirements.md)）で対応
