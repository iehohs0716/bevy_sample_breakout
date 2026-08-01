# SupabaseのGraphQL対応・KVストレージ誤解・DB専用利用パターン

日付: 2026-07-31

本ドキュメントは、Supabase自体の仕組みを理解するための一問一答セッションの記録。
実装作業ではなく、技術理解を深めるための解説であり、本リポジトリの設計方針を変更するものではない。

## 0. 位置づけ

`docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md` §5.3 は、フロントから
Supabase固有の仕組み（PostgREST直叩き・RLSの`auth.uid()`）を直接叩かせず、自前API層に閉じ込める
方針を確定済み。本ドキュメントで扱う「SupabaseをDB専用として使うパターン」（§4）は、この既存方針と
親和性が高いという位置づけの整理であり、新たな決定事項ではない。

## 1. GraphQL対応 = KVストレージが持てる、ではない

**誤解**: 「SupabaseはGraphQLに対応しているから、KVストレージのような柔軟なデータの持ち方ができる」

**訂正**: GraphQLは「クエリの書き方（クエリ言語）」であり、データの持ち方（リレーショナルかKVか）とは
**独立した軸**。両者を混同しやすいが別レイヤーの話。

| 軸 | 例 |
|---|---|
| クエリ言語（フロントとの通信方式） | REST / GraphQL / gRPC 等 |
| データストア（裏側の保存形式） | リレーショナルDB（Postgres, MySQL） / KVストア（DynamoDB, Redis） / ドキュメントDB（MongoDB, Firestore） |

SupabaseのGraphQL対応は `pg_graphql` というPostgres拡張が、既存のPostgresテーブル構造を
自動でGraphQLスキーマに変換して公開しているだけであり、**裏側のデータストアは変わらず
PostgreSQL（リレーショナルDB）のまま**。GraphQLでクエリできるようになっても、テーブル定義・
外部キー・正規化といったリレーショナルの制約はそのまま残る。

KVストレージが欲しい場合にSupabase（Postgres）でできることは、あくまで「Postgresの中に
JSONBカラムを持たせて疑似的にKV的な使い方をする」というワークアラウンドに過ぎず、
Supabase自体がKVストア機能（DynamoDBのような）を持っているわけではない。

## 2. pg_graphqlが生成するスキーマはポータブルではない

pg_graphqlがテーブルから自動生成するGraphQLスキーマには、pg_graphql独自の実装上の規約が
複数含まれる。

- ミューテーション名: `insertIntoXxxCollection` のような命名規則
- ページネーション: Relay風の `connection` / `edges` / `node` 構造
- フィルタ構文: `filter: { column: { eq: ... } }` のようなオペレータ表現

これらはGraphQL仕様そのものではなく**pg_graphql固有の実装**であり、将来Supabaseから
AWS等へ移管し、自前でGraphQLサーバー（AWS AppSync、Apollo Server等）を別途立てた場合、
フロント側のGraphQLクエリはほぼ確実に書き直しが必要になる。「GraphQLを使っているから
バックエンドの移行が楽」にはならない点に注意。

## 3. Postgresスキーマから自動でGraphQLサーバーを立てる仕組みは他にも存在する

| ツール | 概要 | 移管のしやすさ |
|---|---|---|
| **pg_graphql**（Supabase採用） | Postgres拡張。SupabaseのAPIゲートウェイ経由で公開される | 低〜中。OSS自体は移植可能だが、周辺のHTTPサーバー部分（ゲートウェイ）を自前で組む必要がある |
| **Hasura** | Postgresに接続するとテーブル・リレーションから自動でGraphQL API（権限システム・realtime subscription込み）を生成するOSS。Docker等で自前ホスト可能 | 高。Postgresさえあればどこでも動くため、Supabase→AWS RDS等への移管がしやすい |
| **PostGraphile** | Postgresのスキーマ（テーブル・コメント・関数）からGraphQL APIを生成するNode.js製ツール。プラグインで拡張しやすい | 高。Hasuraと同様、Postgres自体があればどこでも動く |

pg_graphql自体はOSSであり理論上は自分のPostgresに組み込むこともできるが、Supabaseはこれを
独自のAPIゲートウェイ経由で公開しているため、Supabase込みで「同じように使う」には
周辺のHTTPサーバー部分を自分で組む手間が発生する。移管のしやすさで言うと
**Hasura／PostGraphile > pg_graphql（Supabase込み）**という序列になる。

## 4. Supabaseを「DBとAPIだけ」使うパターン

Auth／Storage／Realtime／Edge Functionsを一切使わず、フロントから自動生成REST/GraphQLも
直接叩かず、自前API層の裏側で単なるPostgresクライアントとして繋ぐだけ、という使い方は
実際によくあるパターンである。

この場合、Supabaseならではの価値（RLSベースのセキュリティ、Auth統合、自動生成API）は
ほぼ使わないことになるため、実質的には素のマネージドPostgres（AWS RDS、Neonなど）を
使うのとほぼ同じ位置づけになる。

本リポジトリはまさにこの「DBとしてのみSupabaseを使う」方向性
（`docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md` §5.3の自前API層方針）と
親和性が高い。§5.3ではPostgREST直叩き・RLSの`auth.uid()`連携をフロントに結合させない方針を
すでに確定しており、GraphQL/pg_graphqlについても同様に「使わない」選択と整合する。

## 関連ドキュメント

- `docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md` §5.3（Supabase固有機能を
  フロントに結合させないポータビリティ方針）
- `docs_bevy_sample/20260731_auth0-supabase-third-party-auth.md`（認証をAuth0に任せる場合の
  Supabase連携）
