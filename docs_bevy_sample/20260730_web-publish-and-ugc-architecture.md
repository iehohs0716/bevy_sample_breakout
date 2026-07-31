# ブロック崩し Web 公開 & UGC 基盤 設計書

> 本書は当日 14:44 時点の初期ドラフトを、同日中の検討・検証結果を踏まえて更新したもの
> （バックエンド選定の確定、DynamoDB 採用の確定、ローカル動作確認の実施を反映）。
> 分割・整理された正式版は `doc_arch/`（`overview.md` 等）側で管理しており、本書は
> この日の議論のスナップショットという位置づけ。

## 0. 目的とスコープ

現在の `bevy_sample` は、Bevy 製ブロック崩し（`game_engine`）を WASM ビルドし、
React フロント（`frontend`）から起動する **単一レベル・クライアント完結**のサンプルである。
これを次の 2 段階で拡張する。

1. **Web 公開**: 静的サイトとして誰でもアクセスできる状態にする（ホスティング／CI/CD／配信最適化）。
2. **UGC（User Generated Content）基盤**: 誰でも「画像＋パラメータ」を用意すれば自分のブロック崩し
   レベルを作成・公開でき、他の人はそれを一覧から選んでプレイできるようにする。

本書はこの 2 段階を見据えたフル設計書であり、以下を確定事項として前提に置く。

| 項目 | 決定事項 |
|---|---|
| フロントホスティング | Cloudflare Pages |
| バックエンド（BaaS） | **Supabase に決定**（§5.1。Firebase との比較検討は完了済み） |
| レベルパラメータの保存 | **Supabase（Auth/Storage）＋ DynamoDB のハイブリッドに決定**（§5.2 案B。DynamoDB は使わない案Aを廃案とし、当初方針どおり採用する） |
| ローカル開発 | Docker Compose で完結させる。バックエンドはローカルではモック／ローカル互換スタックで代替する。Supabase 側は Supabase CLI ではなく、`db`＋`rest`（PostgREST）のみを含む最小構成の自前 `docker-compose.yml` で運用することを確認済み（§6） |
| **ポータビリティ方針** | **将来 Supabase から Neon 等の別 Postgres サービスへ移行する可能性があるため、特定ベンダー固有の仕組み（PostgREST 直叩き・RLS の `auth.uid()` 連携等）にフロントを直接結合させない（詳細 §5.3）** |

未決事項（本書のレビューで確定させたい点）は末尾 §12 にまとめる。

---

## 1. 現状のアーキテクチャ（As-Is）

```
[ブラウザ]
  └─ React (frontend/) ── window.__BREAKOUT_CONFIG__ に初期化パラメータをセット
       ├─ backgroundBytes / backgroundMime   … 背景画像バイト列
       ├─ bricks: [{x,y}, ...] / cellSize    … ブロック配置
       └─ brickImage: { bytes, mime }        … ブロック用画像
       │
       └─ WASM (breakout.js / breakout_bg.wasm) を起動
            └─ game_engine (Bevy) が起動時（injection.rs）に上記を読み取り、
               無ければコンパイル時定数（config.rs）にフォールバックして描画
```

重要なのは、**現状の `injection.rs` がすでに「1レベル分のデータ構造」を定義している**点である。
これは以下のように UGC のレベルスキーマにほぼそのまま転用できる。

- 背景画像バイト列＋MIME
- ブロック配置（座標配列 + セルサイズ）
- ブロック用画像バイト列＋MIME

一方、パドルサイズ・ボール速度・初期ライフ数などは `config.rs` の Rust 定数にハードコードされており、
現時点では JS から注入不可能。UGC の MVP では「背景・ブロック配置・ブロック画像」のみを
可変パラメータとして扱い、ゲームバランス系パラメータの外部化は将来拡張とする（§3.2）。

現状の課題（Web 公開前提で見たときの不足）：

- CI/CD なし、デプロイ設定なし
- ルート README なし
- 背景画像は「S3 等 CORS 許可済みホスト」から React が fetch する設計だが、本番ドメイン・CORS 設定は未確定
- バックエンド／永続化層が存在しない（＝レベルを保存・共有する仕組みがない）

---

## 2. 拡張後の全体アーキテクチャ（To-Be）

```
                         ┌─────────────────────────┐
                         │      Cloudflare Pages     │  静的配信 (React + WASM)
                         └────────────┬──────────────┘
                                      │ HTTPS（自前 API 契約のみ）
                         ┌────────────▼──────────────┐
                         │  自前 API 層               │  Cloudflare Pages Functions
                         │  (Facade)                 │  フロントはこことしか話さない
                         └────────────┬──────────────┘
                                      │ 標準 SQL / Storage アダプタ経由
                 ┌────────────────────┼─────────────────────┐
                 │                    │                     │
        ┌────────▼────────┐ ┌─────────▼─────────┐  ┌────────▼────────┐
        │ Auth             │ │ Storage             │  │ Postgres          │
        │ Supabase Auth    │ │ Supabase Storage     │  │ (レベルパラメータ) │
        │ (JWT 発行のみ)    │ │ (画像: 背景/ブロック) │  │ Supabase Postgres │
        │                  │ │                     │  │ ※将来 Neon 等へ差替 │
        └──────────────────┘ └────────────────────┘  └───────────────────┘
```

処理の流れ（UGC MVP）:

1. **作成**: ユーザーがレベルエディタ（フロント側の新機能）で背景画像・ブロック配置・ブロック画像を
   用意 → 自前 API 層に送信 → API 層が画像を Storage に保存し、パラメータ（座標配列・cellSize・
   画像参照 URL・メタ情報）を Postgres に保存する。
2. **一覧**: レベル一覧画面が自前 API 層（`GET /api/levels`）から公開レベルのメタ情報
   （タイトル・サムネイル URL・作者等）を取得して表示。
3. **プレイ**: 選択されたレベルの ID を自前 API 層に渡し、Postgres・Storage を引いた結果を
   `window.__BREAKOUT_CONFIG__` に整形して詰める → 既存の `injection.rs` がそのまま読み込む
   （**game_engine 側の変更は不要**、フロントの「どこからパラメータを取得するか」だけが変わる）。

この設計の利点は 2 つある。

- **Bevy/WASM 側を一切改修せずに UGC 基盤を追加できる**こと。既存の
  `window.__BREAKOUT_CONFIG__` 契約を「フロントが静的に組み立てる」から「フロントがバックエンドから
  取得して組み立てる」に差し替えるだけで済む。
- **フロントが Supabase 固有の仕組み（PostgREST の URL 規約・RLS の `auth.uid()`・
  Storage SDK）を一切知らない**こと。フロントが知っているのは自前 API 層の契約
  （`GET /api/levels` 等）だけであり、Supabase から Neon 等へ移行する際の影響範囲は
  API 層の内部実装に閉じる（詳細は §5.3）。

---

## 3. 機能要件

### 3.1 MVP スコープ

- [ ] レベル一覧の閲覧（未ログインでも閲覧・プレイ可能＝読み取りは公開）
- [ ] レベルの作成・投稿（背景画像・ブロック配置・ブロック画像・タイトル）
- [ ] 投稿には最低限の認証が必要（匿名連投・荒らし対策。認証方式は §5 参照）
- [ ] 画像はアップロード時にサイズ・形式を検証（§10）

### 3.2 将来拡張（本書では設計のみ、実装スコープ外）

- パドルサイズ・ボール速度・初期ライフ数などゲームバランス系パラメータの外部化
  （`config.rs` の該当定数を injection 対象に追加）
- いいね／プレイ回数によるランキング・ソート
- タグ・検索
- レベルの通報・非表示（モデレーション）
- ユーザープロフィール・自分の投稿一覧

---

## 4. データモデル

### 4.1 レベル定義スキーマ（案）

```jsonc
{
  "levelId": "lvl_01HXYZ...",        // ULID/UUID
  "title": "初心者向けピラミッド",
  "authorId": "usr_...",             // 認証ユーザーID（匿名投稿を許すなら nullable）
  "visibility": "public",            // "public" | "unlisted"
  "createdAt": "2026-07-30T00:00:00Z",
  "background": {
    "imageUrl": "https://.../levels/lvl_.../background.png",
    "mime": "image/png"
  },
  "brickImage": {
    "imageUrl": "https://.../levels/lvl_.../bricks.png",
    "mime": "image/png"
  },
  "cellSize": { "width": 50, "height": 30 },
  "bricks": [{ "x": -200, "y": 100 }, { "x": -150, "y": 100 }, "..."],
  "stats": { "playCount": 0, "likeCount": 0 }
}
```

`background` / `brickImage` を「バイト列」ではなく「URL」で持つ点が `injection.rs` の現行仕様
（`backgroundBytes: Uint8Array`）との差分。フロント側でレベル取得後に画像を `fetch` してバイト列化し、
既存の `window.__BREAKOUT_CONFIG__` 形式に変換してから Bevy に渡す（変換はフロントの責務、
game_engine は無改修）。

### 4.2 保存先の割り当て

| データ | 保存先 | 理由 |
|---|---|---|
| `background.imageUrl` / `brickImage.imageUrl` が指す実体（画像バイナリ） | Supabase Storage | 大容量バイナリはオブジェクトストレージが適する |
| レベル定義本体（`bricks` 配列・`cellSize` 等） | **DynamoDB**（§5.2 案B で確定） | `levelId` キーでの読み取り中心アクセスに合う |
| レベルのメタ情報（`title` / `authorId` / `visibility` / `createdAt` / `stats` 等） | Supabase Postgres | 一覧・検索・集計等のリレーショナルなクエリに使う |
| 認証情報・セッション | Supabase Auth | 自前実装しない |

### 4.3 画像配信と CORS

既存の `docs_bevy_sample/20260711_external-image-cors-and-formats.md` の教訓（`curl` で
200 が返ってもブラウザの `fetch()` は ACAO ヘッダが無いとブロックする）は、Supabase Storage
採用後も同様に当てはまる。**別途 S3 等の画像ホスティングを新設する必要はなく、Supabase
Storage 1 箇所に画像を集約してよい**が、以下は明示的に対応する。

- レベル画像を置くバケットは **public** にする（読み取りは未認証で可）。public バケット自体は
  追加の CORS 設定なしでも GET 自体は通るが、ブラウザからの `fetch()` を確実に通すため
  バケットの CORS 設定（`allowedOrigins`）に本番ドメイン（Cloudflare Pages の URL）と
  ローカル開発オリジン（`http://localhost:5173` 等）を明示登録する。
- アップロードは `supabase-js` の Storage SDK 経由に統一する（presigned URL を自前で組み立てる
  方式は CORS エラーの報告例が複数あり、避ける）。
- 「Supabase Storage の設定上は通っているはずが実ブラウザでは弾かれる」という既存の罠がある以上、
  **本番相当のドメイン・オリジンで Playwright 実機確認するまでは CORS 対応完了とみなさない**
  （`CLAUDE.md` の「実ブラウザで検証する」方針をそのまま踏襲）。

---

## 5. バックエンドアーキテクチャ選定

### 5.1 比較検討: Firebase vs Supabase

| 観点 | Firebase | Supabase |
|---|---|---|
| データストア | Firestore（NoSQL, ドキュメント型） | Postgres（RDB, SQL フル機能） |
| クエリ柔軟性 | 複合クエリ・集計に制約あり | JOIN・集計・全文検索など SQL の柔軟性をフルに使える |
| Storage | Firebase Storage（GCS ベース） | Supabase Storage（S3 互換 API） |
| Auth | Firebase Auth | Supabase Auth（GoTrue） |
| ベンダー | Google Cloud（クローズド） | OSS・セルフホスト可能（Vercel/AWS/自前どこでも） |
| **データの移植性** | Firestore は独自のドキュメント型 API。他社への移行はデータモデルの全面書き換えが必要 | Postgres は業界標準の SQL。Neon・RDS・自前ホスティング等、他の Postgres 互換サービスへ `pg_dump`/`pg_restore` ベースでほぼそのまま移行できる |
| **ローカル開発環境** | **Firebase Local Emulator Suite**（公式。Auth/Firestore/Storage 等を模倣） | **Supabase CLI (`supabase start`)** — 本番と同一の OSS コンポーネント（Postgres, GoTrue, PostgREST, Storage API, Kong）を Docker で丸ごと起動 |
| ローカルー本番パリティ | エミュレータはあくまで「模倣」。Firestore の一部挙動・セキュリティルールのエッジケースで本番と差異が出ることがある | ローカルは「模倣」ではなく本番と**同一のソフトウェア**を動かすため差異が原理的に少ない |

**推奨: Supabase。**

理由:

1. ユーザーが「Docker Compose で完結させたい」という要件を明言しており、Supabase CLI は
   まさに Docker Compose ベースでローカルスタックを構成する設計のため親和性が高い。
2. レベル一覧・検索・将来のいいね機能・ランキングなど、リレーショナルなクエリが今後増える
   見込みが高く、Postgres の方が長期的に無理が少ない。
3. Postgres は業界標準であり、将来 Supabase から Neon 等の別サービスへ「データを」移すこと自体は
   容易。ただし「移した先で今までどおり動くか」は §5.3 のアクセス方式の設計次第で決まる
   （Postgres を選ぶだけではロックインは防げない）。

### 5.2 KVS（DynamoDB）の位置づけ【確定】

当初想定されている「レベルパラメータ用 KVS として DynamoDB」について、Supabase 採用を前提に
案A（DynamoDBを使わずPostgresのみ）・案B（Supabase＋DynamoDBのハイブリッド）の2案を検討したが、
**案B（Supabase の Auth・Storage ＋ DynamoDB でレベルデータ本体を持つハイブリッド構成）を
採用することが確定した。** 当初方針どおり DynamoDB を使う。

構成の役割分担:

- **Supabase（Postgres）**: 認証（Auth）・画像ストレージ（Storage）・レベルのメタ情報
  （`authorId` / `visibility` / `createdAt` / 集計値など、リレーショナルなクエリ・
  一覧表示・検索に使うもの）を担当。
- **DynamoDB**: レベル定義本体（`bricks` 配列・`cellSize` 等、`levelId` をキーにした
  読み取り中心のデータ）を担当。プレイ時に叩かれる「1レベル分をキーで引く」アクセスパターンに
  DynamoDB のキーバリュー特性がよく合う。

この役割分担により、案Bで懸念していた「GCP/Supabase と AWS を横断する運用の複雑さ」は
次のように整理する:

- ローカル開発では Supabase 側（`db`+`rest`）と `amazon/dynamodb-local` を、
  それぞれ独立した `docker-compose.yml` として管理してよい（§6 参照。実際に Supabase 側は
  最小構成で動作確認済み）。
- 本番の AWS 側 IAM／課金管理は別途必要になるが、DynamoDB は元々の要件であり許容する。
- §5.3 のポータビリティ方針（フロントは自前 API 層としか話さない）は DynamoDB 側にも
  同様に適用する。フロントも自前 API 層も DynamoDB の SDK 特有の呼び方を意識せず、
  自前 API 層の内部実装（リポジトリ層）に DynamoDB アクセスを閉じ込める。

### 5.3 ポータビリティ確保の方針（Facade で Supabase を隠蔽する）

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

**方針**: フロントエンドは Supabase の SDK・PostgREST・Storage SDK を **直接叩かない**。
代わりに、アプリ自身が所有する薄い API 層（Cloudflare Pages Functions を想定。フロントと
同じ Cloudflare Pages 上にデプロイでき、追加インフラが増えない）を挟み、**フロントはこの
自前 API 層としか通信しない**。Supabase 固有の呼び出し方はすべてこの API 層の内部実装に
閉じ込める。実装としては一般的な **Facade パターン**（＋ Storage 部分は Adapter パターン）
そのものであり、目新しい仕組みを導入するわけではない。

| 要素 | Supabase 標準の使い方 | ロックインの度合い | ポータブルにする方法 |
|---|---|---|---|
| データ本体 | Postgres テーブル | 低（標準 SQL） | Supabase 固有の拡張機能には依存しない標準 SQL のみで組む |
| データアクセス方式 | PostgREST 直叩き + RLS | **高**（Supabase 固有の URL 規約・`auth.uid()`） | API 層が標準的な SQL ドライバでクエリを発行し、認可判定も API 層側のコードで行う（RLS に丸投げしない） |
| 画像ストレージ | `supabase-js` Storage SDK 直叩き | 中〜高 | API 層がアップロード／配信を仲介するアダプタを持ち、フロントは自前 API のエンドポイントしか知らない |
| 認証 | Supabase Auth（GoTrue）が発行する JWT | 中（JWT 形式自体は標準的だが発行元は Supabase 固有） | API 層は標準的な JWT 検証のみ行い、発行元（Issuer）を差し替え可能な作りにする |

**トレードオフの明示**: この方針は、Supabase の「サーバーレスで自前バックエンド不要」という
最大のメリットを一部手放し、API 層のコードを自前で書く分、開発速度は PostgREST 直叩きより
落ちる。しかし「将来 Neon 等へ移行する可能性がある」というポータビリティ要件の優先度が
開発速度より高いため、本書ではこちらを採用する。以降 §7（API設計）・§8（フロント変更点）は
この方針を前提に記述する。

---

## 6. ローカル開発環境（Docker Compose）

### 6.1 各サービスのローカル互換手段

| 本番サービス | ローカル代替 | 備考 |
|---|---|---|
| Supabase（Auth/Storage/DB） | 自前の `docker-compose.yml`（後述 6.2） | Supabase CLI (`supabase start`) は採用しなかった（6.2 参照） |
| DynamoDB（§5.2 案B で確定） | `amazon/dynamodb-local`（AWS 公式 Docker イメージ） | ポート 8000 でローカル互換 API を提供（後述 6.3） |
| フロント（React/WASM） | `vite dev` / `vite preview` | 通常の Vite dev server で十分 |
| 自前 API 層（Cloudflare Pages Functions） | `wrangler pages dev` | §5.3 の API 層をローカル実行。Postgres 接続先はローカル Supabase スタックを向ける |

### 6.2 実際に構築・動作確認した構成（Supabase CLI ではなく自前 docker-compose）

当初 6.1 で想定していた「Supabase CLI (`supabase start`) にまかせる」方式は採用しなかった。
代わりに、公式のセルフホスティング用テンプレート（`supabase/supabase` リポジトリの `docker/`
フォルダ）から**必要なサービスだけを取り出した、最小構成の `docker-compose.yml`**を
リポジトリルートに作成し、実際に動作確認まで済ませた。

```
docker-compose.yml（リポジトリルート）
├─ db    (Postgres)      ← supabase/postgres イメージ。roles.sql のみ初期化スクリプトとして残す
└─ rest  (PostgREST)     ← レベル一覧/詳細のCRUD。ホストの 8001 番ポートで直接応答
```

`auth`（GoTrue）・`kong`・`storage`・`studio`・`realtime`・`supavisor`・`functions` は
**意図的に定義していない**。理由は次の2つ:

1. 認証は Supabase Auth を使うかどうか自体が別途検討中（`doc_arch/backend.md` §5.4、
   Auth.js を自前 API 層に内包する案）であり、少なくとも現時点の検証スコープでは不要。
2. 「本当に使うものだけを置く」という方針のもと、`db`＋`rest` の設定から実際に参照されて
   いるファイル・環境変数だけを残すよう棚卸しした（`supabase-local/volumes/db/roles.sql`
   のみ、`.env`も参照されている10項目程度のみに整理済み）。

動作確認は `sandbox/supabase-rest-client-check/check_connectivity.py`（Python, PEP723形式の
単体スクリプト）で行った。Supabase の REST API（PostgREST、`apikey`/`Authorization` ヘッダに
anon key を載せるだけ、Supabase Auth は経由しない）に対して `INSERT` → 別プロセスでの
`SELECT` を実行し、書き込みが正しく永続化されることを確認済み。

なお、一度作業中に `docker compose up -d`（サービス名を指定しない全起動）が実行され、
`auth` を含むフルスタックが誤って立ち上がった事故があったが、`docker-compose.yml` 自体を
`db`＋`rest`のみの定義に切り詰めたことで、**同じコマンドを打っても物理的にフルスタックは
起動できない**状態にしてある。

### 6.3 DynamoDB Local の追加（§5.2 で確定済み・実装時に追加）

```yaml
# docker-compose.yml とは別に、DynamoDB Local を追加する場合の例
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

§5.2 で案B（Supabase＋DynamoDBのハイブリッド）が確定したため、この節は「採用する場合の
差分」ではなく**採用が前提の構成**である。レベルデータ本体を扱う自前 API 層の実装に着手する
タイミングで、上記を Supabase 側の `docker-compose.yml` とは別ファイルとして追加する。

---

## 7. API設計（自前 API 層経由・案A前提）

§5.3 の方針に基づき、フロントは PostgREST や Storage SDK を直接叩かない。Cloudflare Pages
Functions 上に置く自前 API 層が、フロントから見た唯一の窓口になる。

| 操作 | フロントから見た契約（自前 API） | API 層の内部実装 |
|---|---|---|
| レベル一覧取得（公開分のみ） | `GET /api/levels` | Postgres へ標準 SQL（`SELECT ... WHERE visibility = 'public' ORDER BY created_at DESC`）を発行 |
| レベル詳細取得 | `GET /api/levels/:id` | 標準 SQL で 1 件取得 |
| レベル投稿 | `POST /api/levels`（JWT を `Authorization` ヘッダで送付） | JWT 検証 → バリデーション（§10）→ 画像を Storage アダプタ経由で保存 → レベル行を `INSERT` |
| 画像アップロード | `POST /api/levels` に画像を含めて送信（別エンドポイントに分けても可） | API 層が受け取り、Storage アダプタ（現状: Supabase Storage 実装）経由で保存し URL を返す |

この API 層が依存してよいのは「標準的な Postgres 接続（SQL ドライバ）」と「差し替え可能な
Storage アダプタのインターフェース」だけであり、PostgREST の URL 規約にも RLS の
`auth.uid()` にも依存しない。認可判定（投稿者本人か等）は API 層のコードで明示的に行う。

これにより、Postgres の接続先を Supabase → Neon に切り替える作業は「API 層内の接続文字列
（および必要ならドライバ）を変更するだけ」に閉じ込められ、フロント・URL 契約・データモデルは
無変更で済む。同様に画像ストレージを Supabase Storage → S3/R2 に切り替える場合も、
Storage アダプタの実装差し替えだけで済む。

（実装上の注意）Cloudflare Pages Functions（Workers ランタイム）から Postgres への接続方式
（TCP 直結の可否、Neon/Supabase 各社が提供する HTTP 経由ドライバの利用要否）は各社の対応状況に
依存するため、実装着手時に個別検証が必要（§12 未決事項）。

---

## 8. フロントエンド変更点

- 画面追加: レベル一覧画面 / レベル作成（エディタ）画面 / プレイ画面（既存 `BevyGame.tsx` を流用）
- レベル一覧・詳細・投稿はすべて自前 API 層（`/api/levels` 等）を `fetch` する。
  `supabase-js` で DB（PostgREST）や Storage を直接叩くコードはフロントに書かない
  （§5.3）。
- `BevyGame.tsx` 起動前に、選択された `levelId` から自前 API 経由でレベル JSON ＋画像を取得し、
  既存の `window.__BREAKOUT_CONFIG__` 形式（`backgroundBytes` / `bricks` / `brickImage` の
  バイト列形式）へ変換するアダプタ層を追加する。**game_engine 側の改修は不要。**
- 認証（ログイン・サインアップ）フロー自体は例外的に Supabase Auth の JS SDK を使ってよい
  （トークン発行はどの Auth プロバイダを選んでも provider 固有の処理になるため）。
  ただし発行された JWT は自前 API 呼び出しの `Authorization` ヘッダに載せるだけで、
  それ以外（DB・Storage）の SDK 呼び出しはフロントから行わない。
  ※この「例外」自体をなくし、Google/GitHub の OAuth を Auth.js（自前 API 層に内包）に
  担当させ、Supabase は素の Postgres としてのみ使う案を検討中（`doc_arch/backend.md` §5.4、
  `docs_bevy_sample/20260730_authjs-oauth-on-cloudflare-pages-functions.md`）。採用が
  決まればこの箇条書きは置き換える。

---

## 9. ホスティング・CI/CD

- フロント: Cloudflare Pages（Git 連携で push 時自動デプロイ）
- ビルド: `pnpm build:wasm` → `tsc -b` → `vite build`（既存の `frontend/package.json` の
  `build` スクリプトをそのまま CI で実行）
- CI: GitHub Actions で `cargo check --target wasm32-unknown-unknown` → `pnpm build` を実行し、
  Cloudflare Pages の Git 連携ビルドと同じ手順を PR 時にも検証する
- `wasm-bindgen-cli` のバージョンを `Cargo.lock` と一致させる手順を CI にも明記（既存の
  `CLAUDE.md` に記載のローカル制約と同じ）
- Supabase: `supabase db push` によるマイグレーション適用を CI/CD パイプラインに組み込む

---

## 10. セキュリティ・モデレーション

- 画像アップロード: サイズ上限・MIME ホワイトリスト（png/jpeg/webp）をクライアント＋
  サーバー（Storage ポリシー / Edge Function）の両方で検証
- ブロック座標: 異常値（アリーナ範囲外・極端な個数）をサーバー側でも検証し、Bevy 側に
  不正な大量データを渡さない
- 認可判定は自前 API 層のコードで行う（§5.3）。`visibility = 'public'` のみ未認証で
  読み取り可、書き込みは投稿者本人のみ許可、というルールを API 層に実装する。
  Postgres の RLS は「API 層のバグに対する保険」として追加で有効化してもよいが、
  それ単体を認可の主体にはしない（RLS に依存すると Neon 等への移行時に丸ごと作り直しになるため）
- CORS: Supabase Storage バケットの `allowedOrigins` を本番ドメイン・開発オリジンに限定登録する
  （詳細は §4.3）。ワイルドカード許可はしない
- モデレーション: MVP では通報機能なし。公開前提での運用リスクとして明記し、
  将来拡張（§3.2）で対応

---

## 11. 段階的ロードマップ

| フェーズ | 内容 |
|---|---|
| 0（現状） | 単一レベル・クライアント完結の静的サイト |
| 1 | Web 公開のみ（Cloudflare Pages + CI/CD、バックエンドなし） |
| 2 | UGC 基盤 MVP（Supabase: Auth + Storage + Postgres、DynamoDB: レベルデータ本体、レベル投稿・一覧・プレイ） |
| 3 | スケール対応（DynamoDB 側の読み取りキャパシティ調整等） |
| 4 | いいね・検索・タグ・モデレーション等の拡張機能 |

---

## 12. 未決事項・要確認

- [x] ~~DynamoDB を本当に本番採用するか~~ → **決定済み**（§5.2 案B採用。Supabase＋DynamoDB
      のハイブリッド）
- [x] ~~ローカルで Supabase をどう動かすか~~ → **決定済み**（§6.2。Supabase CLI ではなく
      `db`＋`rest`のみの自前 docker-compose。実際に動作確認済み）
- [ ] 投稿に認証を必須にするか、匿名投稿を許容するか（荒らし対策とのトレードオフ）
- [ ] Supabase を Supabase 社のクラウド版で使うか、自前セルフホストするか（コスト・運用負荷が変わる）
- [ ] 画像・レベルデータの上限サイズ／点数（ブロック数上限など）の具体値
- [ ] モデレーション（通報・削除）を MVP に含めるか、後回しにするか
- [ ] Cloudflare Pages Functions から Postgres（Supabase／将来の Neon）への接続方式
      （TCP 直結か、各社提供の HTTP 経由ドライバか）は実装着手時に個別検証が必要（§7）
- [ ] 認証プロバイダも将来差し替える可能性を見込むか（Auth.js を自前 API 層に内包し
      Supabase は素の Postgres としてのみ使う案を検討中、§8 参照）
- [ ] DynamoDB のテーブル設計（キー設計・GSI要否）は §5.2 の役割分担を前提に別途詳細化が必要
