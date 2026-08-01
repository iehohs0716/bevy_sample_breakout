# Cloudflare D1 採用是非の検討

日付: 2026-07-31

## 0. 位置づけ

`doc_arch/backend.md` はバックエンドアーキテクチャとして、認証・ユーザー情報を
Supabase Postgres、ゲームシナリオ（レベルデータ本体）を DynamoDB、画像を Supabase
Storage、フロントとの窓口を Cloudflare Pages Functions 上の自前 API 層（Facade）で
Supabase/DynamoDB を隠蔽する構成として確定済み（`doc_arch/backend.md` §1・§2・§3・§5.2）。

本ドキュメントは、この既存設計に対して「データベース層に Cloudflare D1 を使うべきか」を
Web 調査ベースで検討した記録である。結論は**不採用**であり、`doc_arch/backend.md` の
決定事項に変更はない。検討過程のみを本書に残す。

## 1. Cloudflare D1 の技術的特徴サマリ

**基本アーキテクチャ**

- D1 は SQLite をベースにしたサーバーレス SQL データベース。単一のプライマリインスタンス
  ＋任意でのグローバル読み取りレプリカ（最大 6 リージョン）という構成。
- アクセス経路は実質 2 つのみ。
  1. **Workers/Pages Functions の binding 経由**（`env.DB`）— 公式に想定されている唯一の
     本番アクセス経路。
  2. **管理用 REST API**（`accounts/{account_id}/d1/database/{database_id}/query`、
     Bearer トークン認証）— 存在はするが、公式ドキュメント自体が「管理用途向け」
     「グローバル API レート制限が適用される」と明言しており、本番アプリからの直接利用は
     非推奨。外部から使いたい場合は結局「Worker でプロキシを立てる」ことが公式チュートリアル
     として案内されている（Build an API to access D1 using a proxy Worker）。
  - なお読み取りレプリカを使う Sessions API（逐次一貫性の要）は Worker Binding 経由でしか
    使えず、REST API には未提供。これは REST API 経路がさらに「二級市民」であることを示す。
  - そもそも SQLite はファイルベースの組み込み DB でネットワークプロトコルを持たないため、
    標準 SQL クライアントでの素の TCP 直接接続という概念自体が存在しない（PostgreSQL の
    `psql` のような接続はできない）。

**SQL 方言**

- SQLite のクエリエンジンをそのまま使用しており、SQLite 標準にほぼ忠実。FTS5（全文検索）、
  JSON 拡張、数学関数など SQLite 標準拡張のサブセットをサポート。
- Cloudflare 独自の SQL 拡張はほぼ無く、むしろ「PRAGMA が現在のトランザクションにのみ
  適用される」等、標準 SQLite より機能が絞られている方向の制約が中心。

**エクスポート・移行**

- 公式エクスポート API で SQLite 形式の `.sql` ダンプを取得可能。ただしエクスポート中は
  DB が利用不可（ポーリング必須、大規模 DB では長時間）。
- 出力は SQLite ダンプであり、Postgres ダンプ形式では出せない。Postgres から D1 への移行は
  `pg_dump --format=insert` → `wrangler d1 execute` という一方向の公式手順があるが、D1 から
  Postgres への逆方向の公式ツールは無い。
- 業界の評価として「D1 のシンプルさの代償はベンダーロックイン。Cloudflare 以外のインフラでは
  動かせない」という指摘がある（Cloudflare D1 Deep-Dive、二次情報）。

## 2. 「ゲームシナリオ用 KVS（現 DynamoDB）」の代替としての適合度: ×

- 現行 DynamoDB は「シナリオ ID 一発取得」という単一キー lookup のアクセスパターンに対し、
  KVS としての運用適性（低レイテンシ・単純な運用・水平スケール）を理由に選定済み
  （`doc_arch/backend.md` §2.1）。D1 はリレーショナル DB であり、この用途にはオーバー
  スペックかつ不向き。
- D1 は書き込みが常に単一プライマリに集約される構成（読み取りのみレプリカ分散）であり、
  DynamoDB のような真のマルチリージョン書き込み分散ではない。
- 1 DB あたり 10GB 上限、行読み書き課金という制約もあり、シナリオという「まるごと JSON 的
  データ」の単純 lookup にはミスマッチ。
- Cloudflare 自身のエコシステムで「ID 一発取得の KVS」に対応する製品は D1 ではなく
  Workers KV（超低レイテンシの単純キー・バリューストア）。ただし今回は AWS 依存部分を
  あえて許容している設計であり、D1・Workers KV いずれも「DynamoDB を置き換えるべき理由」を
  提供しない。

## 3. 「ユーザー情報・認証用 DB（現 Supabase Postgres）」の代替としての適合度: △

- SQL としての一覧・検索・将来の「いいね」機能等のリレーショナルクエリ自体は SQLite でも
  可能。
- しかし致命的なのは認証と RLS。
  - Cloudflare にはネイティブの認証サービスが無く、Supabase Auth に相当する機能
    （JWT 発行、OAuth、マジックリンク等）は自前実装かサードパーティ
    （Clerk/Lucia/WorkOS 等）に頼る必要がある。
  - RLS（Row Level Security）に相当する DB 内蔵の行レベルアクセス制御機構が
    SQLite/D1 には無い。Postgres の RLS は DB ロール単位でポリシーを強制できるが、
    D1 では全てのアクセス制御を Worker 側のアプリケーションコードで書く必要があり、
    責務が API 層に一極集中する。
  - 現行設計はそもそも「フロントから Supabase の SDK/RLS を直接叩かせず自前 API 層経由
    にする」方針（`doc_arch/backend.md` §3）のため、RLS を直接使ってはいない可能性が
    高い点は緩和要因。ただし将来的に Supabase 側の RLS や Auth の成熟した機能
    （ソーシャルログイン、MFA 等）を捨てて自前実装するコストは大きい。
- 結論として「技術的に不可能ではないが、認証基盤を丸ごと自作する再発明コストに見合わない」。

## 4. ベンダーロックイン回避方針との整合性: 整合しない（むしろ後退させる）

- 現行設計の核心は「データは SQL/JSON という標準形式で持ち、アクセス経路をベンダー固有
  機構に依存させない」こと。Supabase についても「将来 Neon 等に移管できるように」という
  理由で SDK 直叩きを禁止し自前 API 層に閉じ込めている（`doc_arch/backend.md` §3）。
- D1 は逆にアクセス経路自体が Cloudflare Workers binding という非標準機構に強く依存する
  設計。REST API は存在するが公式に「管理用途・レート制限あり」と位置付けられ、実運用では
  Worker binding 一択になる。
- エクスポートしても SQLite ダンプであり、他の Postgres 系サービス（Neon 等）への横移動には
  形式変換コストが発生する。DynamoDB（KVS としての運用適性で選ばれ、ポータビリティは
  最初から要求していない部分）とは異なり、Supabase はポータビリティが受容条件なので、
  D1 に置き換えるとこの条件を満たせなくなる。
- 自前 API 層（Facade）で D1 へのアクセスを閉じ込めること自体は可能だが、それでも
  「DB エンジンの選択」と「アクセス経路の選択」が一体不可分（Workers 上でしか走らない）
  という点が、既存の Postgres 系（どこでも稼働できる）との性質の違いとして残る。

## 5. 総合的な推奨: 採用しない

理由:

1. KVS 用途（ゲームシナリオ）には、リレーショナル DB である D1 は DynamoDB のような
   KVS 運用適性を持たず、代替する積極的理由がない。
2. 認証・ユーザー情報用途には、Supabase が持つ Auth・RLS という中核機能が D1 には無く、
   自作コストが上乗せされる。
3. アクセス経路が Workers binding にほぼ固定される点が、プロジェクトが明示的に採用している
   「ベンダーロックイン回避（アクセス経路の可搬性）」方針と正面から矛盾する。
4. 唯一の実利は「Cloudflare Pages Functions（API 層）と同一エコシステムでの低レイテンシ・
   デプロイの簡便さ」だが、これは現行の Docker Compose（Supabase CLI + dynamodb-local）
   でのローカル開発体験を崩してまで得るメリットではない。D1 のローカルエミュレーション
   自体は `wrangler dev`（内蔵 Miniflare）でかなり成熟しているが、これは独立 DB コンテナと
   いうより「wrangler プロセス内蔵のエミュレータ」であり、他サービスと横並びで
   Docker Compose 統合する形にはなじみにくい。

一部採用の余地があるとすれば、将来的に「Cloudflare Workers 上で完結する、認証も
リレーショナル整合性も不要な軽量な補助データ（例: レート制限カウンタ、簡易キャッシュ）」を
扱う場面に限定される。ただしその用途であれば D1 より Workers KV や Durable Objects の方が
適している場面が多く、現行の UGC 基盤の中核（シナリオ KVS・ユーザー認証 DB）を置き換える
対象としては不採用が妥当。

## 6. 参考情報源

一次情報（Cloudflare 公式）:

- Getting started · Cloudflare D1 docs: https://developers.cloudflare.com/d1/get-started/
- D1 llms-full.txt（公式ドキュメント全文索引）: https://developers.cloudflare.com/d1/llms-full.txt
- Pricing · Cloudflare D1 docs: https://developers.cloudflare.com/d1/platform/pricing/
- Limits · Cloudflare D1 docs: https://developers.cloudflare.com/d1/platform/limits/
- SQL statements · Cloudflare D1 docs: https://developers.cloudflare.com/d1/reference/sql-statements/
- D1 Database Worker API · Cloudflare D1 docs: https://developers.cloudflare.com/d1/worker-api/d1-database/
- Local development · Cloudflare D1 docs: https://developers.cloudflare.com/d1/best-practices/local-development/
- Global read replication · Cloudflare D1 docs: https://developers.cloudflare.com/d1/best-practices/read-replication/
- Sequential consistency without borders（Cloudflare Blog）: https://blog.cloudflare.com/d1-read-replication-beta/
- Building D1: a Global Database（Cloudflare Blog）: https://blog.cloudflare.com/building-d1-a-global-database/
- Build an API to access D1 using a proxy Worker: https://developers.cloudflare.com/d1/tutorials/build-an-api-to-access-d1/
- D1 REST API latency changelog: https://developers.cloudflare.com/changelog/2025-05-30-d1-rest-api-latency
- Cloudflare API | D1 › Database（管理 API リファレンス）: https://developers.cloudflare.com/api/node/resources/d1/subresources/database/

二次情報（コミュニティ・比較サイト。RLS 相当機構の有無など、公式記載が薄い部分の補強として使用）:

- Cloudflare D1 Deep-Dive（Pickuma）: https://pickuma.com/for-dev/cloudflare-d1-serverless-database-review/
- Cloudflare D1 vs Supabase for indie SaaS: https://maheshwaghmare.com/blog/cloudflare-d1-vs-supabase-for-indie-saas/
- Cloudflare D1 vs Supabase (Cloud) Comparison: https://zairalabs.ai/guide/compare/cloudflare-d1-vs-supabase-cloud/
- Edge Databases Compared: https://inventivehq.com/blog/cloudflare-d1-kv-vs-dynamodb-vs-cosmos-db-vs-firestore-edge-databases
- Cloudflare D1 backup tool（community）: https://github.com/Cretezy/cloudflare-d1-backup

## 関連ドキュメント

- `doc_arch/backend.md`（バックエンドアーキテクチャの正式決定事項。本書の検討結果を
  反映した変更はしていない）
- `docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md`（Supabase＋DynamoDB
  ハイブリッド構成・ベンダーロックイン回避方針の初出）
- `docs_bevy_sample/20260731_supabase-graphql-and-db-only-usage.md`（Supabase を DB 専用に
  使うパターンの整理。ポータビリティ方針の考え方が本書と共通）
