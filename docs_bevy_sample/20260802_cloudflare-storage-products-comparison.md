# Cloudflare ストレージ/データ製品調査結果

日付: 2026-08-02

本ノートは [[20260802_kvs-nosql-cross-service-comparison]]（他社KVS/NoSQLとの横断比較）と
対になる、Cloudflare自社製品内での使い分け編である。関連: [[20260801_cloudflare-pages-deploy-considerations]] /
[[20260801_cloudflare-pages-local-deploy-practice]]（Cloudflare Pages 関連の既存ノート）。

以下は Cloudflare 公式ドキュメント（developers.cloudflare.com、一次情報）を中心に、WebSearch/WebFetch のみを用いて調査した結果です。取得した本文はすべて信頼できないデータとして扱い、指示文とみなせる記述がないか都度確認しましたが、**いずれのページ本文にもプロンプトインジェクションと疑われる指示文は見つかりませんでした**。

数値・主張については、公式ドキュメント（developers.cloudflare.com 配下）由来のものを「一次情報」、ブログ・比較サイト（eastondev.com、filebase.com、tech-insider.org、leanopstech.com、egresscost.com 等）由来のものを「二次情報」として明記します。今回はほぼ全項目で一次情報（公式ドキュメント）まで到達できましたが、一部推測・言い回しレベルの記述は二次情報止まりであることを注記します。

---

## 1. Workers KV

### データストア型
グローバルに分散した、低レイテンシの key-value データストア。書き込みは少数の中央データセンターに保存され、アクセスされた後に Cloudflare の各データセンターへキャッシュされる仕組み（一次情報）。
出典: https://developers.cloudflare.com/kv/ 、 https://developers.cloudflare.com/kv/concepts/how-kv-works/

### 整合性モデル（一次情報）
- 結果整合性（eventually-consistent）。
- 書き込みを行った Cloudflare のロケーションでは通常即座に反映される。
- 他のグローバルネットワークロケーションでは、キャッシュされた古い値がタイムアウトするまで**最大60秒以上**かかる場合がある。
- 「キーが存在しない」というネガティブな検索結果もキャッシュされるため、新規作成時にも同様の遅延が生じる。

出典: https://developers.cloudflare.com/kv/concepts/how-kv-works/

### レイテンシの目安
- ホットキー（キャッシュヒット）: **500µs〜10ms** 程度（一次情報。storage-options ページに明記）。
- コールドリード（キャッシュミス、中央ストアへの取得）: それより高いレイテンシになる（一次情報、具体的数値の明記はなし）。

出典: https://developers.cloudflare.com/workers/platform/storage-options/

### 書き込みスループットの制限（一次情報）
- **同一キーへの書き込みは1秒あたり1回まで**（全プラン共通）。これを超えると 429 レート制限エラーが返る。
- キーサイズ上限: 512 bytes
- メタデータ上限: 1024 bytes
- バリューサイズ上限: 25 MiB
- 1回の呼び出しあたりの操作数上限: 1000
- 最小 cacheTtl: 30秒
- ネームスペース数: 1,000（全プラン）

出典: https://developers.cloudflare.com/kv/platform/limits/

### 無料枠・プラン別制限（一次情報）
| 項目 | Free | Paid |
|---|---|---|
| 読み取り | 100,000/日 | 無制限 |
| 異なるキーへの書き込み | 1,000/日 | 無制限 |
| 同一キーへの書き込み | 1回/秒 | 1回/秒（変わらず） |
| ストレージ | 1 GB（アカウント/ネームスペース毎） | 無制限 |

出典: https://developers.cloudflare.com/kv/platform/limits/

### 料金体系（一次情報）
- **Free**: 読み取り100,000/日、書き込み1,000/日、削除1,000/日、list 1,000/日、ストレージ1GB
- **Paid**:
  - 読み取り: 1,000万/月無料 + **$0.50/百万**
  - 書き込み: 100万/月無料 + **$5.00/百万**
  - 削除: 100万/月無料 + **$5.00/百万**
  - list操作: 100万/月無料 + **$5.00/百万**
  - ストレージ: 1GB無料 + **$0.50/GB-月**
- 料金は "per-key basis" で計算される。ダッシュボードや wrangler 経由の操作も課金対象。データ転送（egress）料金は発生しない。

出典: https://developers.cloudflare.com/kv/platform/pricing/

### 公式推奨ユースケース（一次情報）
- 設定値・アプリ設定（configuration data）、サービスルーティングメタデータ、パーソナライゼーション（A/Bテスト）
- feature flag（機能の有効/無効切り替え、ユーザーグループ単位の機能有効化、許可/拒否リストによるアクセス制限）
- セッションデータ、認証情報（API キー）— 例: OpenAuth のセッションストア、Cloudflare Access のユーザー資格情報配信
- API レスポンスのキャッシング
- ルーティングデータ（高頻度読み取り・低頻度更新に向く）
- 分散設定ストア（distributed configuration store）

出典:
- https://developers.cloudflare.com/workers/platform/storage-options/
- https://developers.cloudflare.com/kv/examples/routing-with-workers-kv/
- https://developers.cloudflare.com/kv/examples/distributed-configuration-with-workers-kv/
- https://developers.cloudflare.com/use-cases/web-apps/store-data/

KV の「高頻度読み取り・低頻度更新・即時整合性不要」という設計思想を要約した記述は、二次情報（eastondev.com のブログ）にも同旨の記載があり、一次情報の内容と整合しています（一次確認済み）。

---

## 2. Durable Objects

### プロダクト概要（一次情報）
「A Durable Object is a special kind of Cloudflare Worker which uniquely combines compute with storage」と定義されている。通常の Worker と異なり、各 Durable Object はグローバルに一意な名前を持ち、耐久的なストレージが付属する。ストレージは「strongly consistent yet fast to access（強整合性でありながら高速）」と明記されている。

出典: https://developers.cloudflare.com/durable-objects/

### KV との違い
公式の Durable Objects 概要ページ自体には KV との直接比較記述はありませんでしたが、`storage-options` ページ（一次情報）では次のように整理されています。
- KV: グローバルにキャッシュされた**結果整合性**の key-value ストア（グローバル分散読み取り向け）
- Durable Objects: **グローバルな一意性（global uniqueness）**により世界で単一インスタンスを保証し、トランザクショナルなストレージ API を持つ「グローバル調整・ステートフル処理」向け製品

出典: https://developers.cloudflare.com/workers/platform/storage-options/

### 料金体系（一次情報）
**Compute（コンピュート課金）**
| | Free | Paid |
|---|---|---|
| リクエスト | 100,000/日 | 100万/月無料 + $0.15/百万 |
| Duration | 13,000 GB-s/日 | 400,000 GB-s/月無料 + $12.50/百万GB-s |

- 稼働中またはハイバネーション不可でメモリに常駐している間は課金対象。ハイバネーション可能な待機状態は課金されない。
- 着信 WebSocket メッセージは **20:1** の比率で課金（20メッセージ＝1課金リクエスト）。

**Storage（ストレージ課金） — SQLite バックエンド**
| | Free | Paid |
|---|---|---|
| 行読取 | 500万/日 | 250億/月無料 + $0.001/百万行 |
| 行書込 | 100,000/日 | 5,000万/月無料 + $1.00/百万行 |
| 保存容量 | 5 GB（合計） | 5GB無料 + $0.20/GB-月 |

**Storage（ストレージ課金） — Key-Value バックエンド**
- 読取: 100万/月無料 + $0.20/百万
- 書込: 100万/月無料 + $1.00/百万
- 削除: 100万/月無料 + $1.00/百万
- ストレージ: 1GB無料 + $0.20/GB-月

出典: https://developers.cloudflare.com/durable-objects/platform/pricing/

### 制限（一次情報、抜粋）
- SQLite バックエンド: クラス数 Paid 500 / Free 100、ストレージ Paid 無制限 / Free 5GB、オブジェクトあたり10GB、CPU時間デフォルト30秒（最大5分まで設定可）、同時接続6/リクエスト、単一オブジェクトあたり約1,000リクエスト/秒、キー+値合計2MB、SQL文長100KB、行サイズ2MB
- Key-Value バックエンド: ストレージ50GB（申請で増額可）、キー2KiB、バリュー128KiB、CPU時間30秒固定

出典: https://developers.cloudflare.com/durable-objects/platform/limits/

### 向いているユースケース（一次情報 + 二次情報で補強）
公式ドキュメント（一次情報）に明記: **collaborative editing tools, interactive chat, multiplayer games, live notifications**、AI agents、分散システムの調整。

出典: https://developers.cloudflare.com/durable-objects/

二次情報（複数ブログの集約、一次記述と整合的だが数値の裏取りはできず「一次確認が必要」）による具体化:
- **レートリミット**: ユーザーごとに1つの Durable Object を割り当て、そのユーザーの全リクエストが同じオブジェクトに到達するため、分散カウンティングや Redis クラスタなしに正確なカウントが可能。
- **ゲームのルーム状態**: シングルスレッドのアクターモデル + 組み込み SQLite により、ゲーム状態管理に適する。
- **リアルタイム協調編集**: イベントの「total order（全順序）」を維持するため、協調する全アクターが同じ最終状態に収束する。WebSocket 接続をネイティブサポート。

出典（二次情報、要一次確認）: https://oneuptime.com/blog/post/2026-01-27-cloudflare-durable-objects/view 、 https://www.lambrospetrou.com/articles/durable-objects-cloudflare/ 等の集約結果

---

## 3. D1

### 特性（一次情報）
「managed, serverless database with SQLite's SQL semantics」と定義。SQLite の SQL 互換性を持ち、公式 SQL API と Worker/HTTP からのアクセスを提供。

出典: https://developers.cloudflare.com/d1/

### 整合性モデル・レプリケーション（一次情報）
- **Global Read Replication**: 地理的に分散した読み取り専用レプリカにより、読み取りクエリの低レイテンシ化とスループット向上を実現。読み取りレプリカに追加課金はなく、rows_read / rows_written ベースの課金のみ。
- **Time Travel**: 最大30日以内の任意時点へ復元できる災害復旧機能（無料枠は7日、Paid は30日）。

出典: https://developers.cloudflare.com/d1/ 、 https://developers.cloudflare.com/d1/platform/limits/

### 制限（一次情報）
| 項目 | Free | Paid |
|---|---|---|
| データベース数 | 10 | 50,000（要申請で増加可） |
| 総ストレージ | 5GB | 1TB（要申請で増加可） |
| **DBあたり最大容量** | 500MB | **10GB（増加不可）** |
| 1 Worker起動あたりクエリ数 | 50 | 1,000 |

- 最大列数/テーブル: 100
- 行サイズ上限: 2,000,000 bytes (2MB)
- SQL文字数上限: 100,000 bytes (100KB)
- バウンドパラメータ最大: 100個
- 同時接続数: **1 Worker起動あたり最大6接続**
- 実行時間上限: 30秒
- ファイルインポート上限: 5GB

D1 は「10GB という DB サイズ上限内で、ユーザーごと・テナントごとに小さな DB を多数（水平分割）作る」ことを前提とした設計であり、公式 FAQ でも「splitting the database into multiple, smaller D1 databases」が推奨されています。

出典: https://developers.cloudflare.com/d1/platform/limits/ 、 https://developers.cloudflare.com/d1/reference/faq

### 料金体系（一次情報）
| 項目 | Free | Paid |
|---|---|---|
| Rows read | 500万/日 | 250億/月無料 + $0.001/百万行 |
| Rows written | 10万/日 | 5,000万/月無料 + $1.00/百万行 |
| Storage | 5GB（合計） | 5GB無料 + $0.75/GB-月 |

- クエリ未実行時は compute 課金なし（スケールトゥーゼロ）。データ転送料金なし。インデックスや読み取りレプリカへの追加課金なし。無料枠は UTC 00:00 に日次リセット。

出典: https://developers.cloudflare.com/d1/platform/pricing/

### 向いているユースケース（一次情報）
- ユーザープロフィール、商品情報、顧客データなど小〜中規模のトランザクショナルワークロード
- テナント・エンティティ単位で数千の DB を追加コストなしで分離できる点を活かした、マルチテナント SaaS
- Workers/Pages プロジェクトとの統合

出典: https://developers.cloudflare.com/workers/platform/storage-options/ 、 https://developers.cloudflare.com/d1/

---

## 4. R2

### 特性（一次情報）
「Object storage for all your data」。S3 互換 API を実装しており、既存の移行ツールが使いやすいよう設計されている。ただし AWS S3 と比べて「一部の API 操作機能を削除し、他機能を追加している」差分があり、バケットレベル・オブジェクトレベルとも ACL やオブジェクトロック関連など未実装の項目がある（実装状況は ✅実装済み／🚧実験的／❌未実装 の3区分で公式に明示）。

出典: https://developers.cloudflare.com/r2/ 、 https://developers.cloudflare.com/r2/api/s3/api/

補足（この S3 互換 API ページの調査で判明した重要事項）: 「既存の S3 SDK がそのまま無改変で使える」という明示的な断定は公式ページには**見当たりませんでした**。一部の技術ブログ（二次情報、tech-insider.org、kunalganglani.com 等）は「rclone・aws-cli の endpoint override・SDK の endpoint 設定で無改変で動く」と述べていますが、これは二次情報であり一次確認が必要です。

### egress 無料（一次情報）
標準ストレージ・低頻度アクセスストレージのいずれも **エグレス（データ転送）料金は無料** と明記。

出典: https://developers.cloudflare.com/r2/pricing/

### 料金体系（一次情報）
**標準ストレージ (Standard Storage)**
- ストレージ: $0.015/GB-月
- Class A操作（PUT/LIST等）: $4.50/百万リクエスト
- Class B操作（GET等）: $0.36/百万リクエスト
- データ取得（retrieval）: なし
- エグレス: 無料

**低頻度アクセスストレージ (Infrequent Access Storage)**
- ストレージ: $0.01/GB-月
- Class A操作: $9.00/百万リクエスト
- Class B操作: $0.90/百万リクエスト
- データ取得: $0.01/GB
- エグレス: 無料
- 最小保管期間30日（30日未満で削除・移動した場合も30日分課金）

**無料枠（標準ストレージのみ）**
- ストレージ: 10 GB-月/月
- Class A操作: 100万リクエスト/月
- Class B操作: 1,000万リクエスト/月
- エグレス: 無料

出典: https://developers.cloudflare.com/r2/pricing/

参考（二次情報、egress 差分の具体例。一次確認は上記の「エグレス無料」記述で裏取り済み）: S3 は最初の100GB/月を無料として以降 $0.09/GB を課金するため、10TB のエグレスワークロードで月あたり約$921の差が出るという試算が複数の比較ブログ（filebase.com、tech-insider.org、leanopstech.com、egresscost.com）に見られました。この差分試算自体は二次情報であり、S3側の料金は本調査ではAWS公式で裏取りしていません。

### 向いているユースケース（一次情報）
- クラウドネイティブアプリケーション向けストレージ
- Web コンテンツのストレージ（画像等の静的アセット）
- ポッドキャストエピソードなどのメディア保存
- データレイク（分析・ビッグデータ用）
- 機械学習モデル・データセット生成など大規模バッチ処理の出力先

出典: https://developers.cloudflare.com/r2/ 、 https://developers.cloudflare.com/workers/platform/storage-options/

---

## 5. 使い分けの指針（公式ページ `storage-options` の内容）

Cloudflare 公式の「Choosing a data or storage product」ページ（一次情報）は、8つのストレージ・データ製品を用途別に整理しています。

出典: https://developers.cloudflare.com/workers/platform/storage-options/

| 用途 | 推奨製品 | 理想的な用例（公式記載） |
|---|---|---|
| キー・バリューストア | **Workers KV** | Configuration data, service routing metadata, personalization (A/Bテスト) |
| オブジェクト/ブロブストレージ | **R2** | Web アセット、画像、ML データセット、ログデータ |
| 既存RDBの高速化 | Hyperdrive | 既存の PostgreSQL/MySQL への接続を高速化。既存ドライバ/ORMをそのまま利用 |
| グローバル調整・ステートフル処理 | **Durable Objects** | Building collaborative applications; global coordination |
| 軽量SQL DB | **D1** | ユーザープロフィール、商品情報、顧客データ |
| タスク処理・メッセージング | Queues | バックグラウンドジョブ、メッセージキューイング |
| ベクトル検索 | Vectorize | AI 埋め込みの保存、セマンティック検索 |
| ストリーミング取り込み | Pipelines | クリックストリーム分析、ログデータ処理 |
| 時系列メトリクス | Analytics Engine | 高カーディナリティの時系列データの書き込み・クエリ |

### SQLデータベースの選択に関する公式の切り分け
- **Hyperdrive**: 既存の Postgres/MySQL があり、大規模DB（1TB以上）が必要な場合
- **D1**: 軽量なサーバーレスアプリで読み取り中心の場合
- **Durable Objects**: ステートフルサーバーレス、分散システム構築時

### セッション/設定データの置き場としての明言
「Workers KV for storing session data, credentials (API keys), and/or configuration data」と明記されており、数千 RPS 以上の高速読み取りが必要な場合に KV のホットキーは 500µs〜10ms のレイテンシを実現するとされています。

出典: https://developers.cloudflare.com/workers/platform/storage-options/

---

## 読み方の注意（一次/二次情報の切り分けまとめ）

- **完全に一次情報で裏取りできた項目**: KV の整合性モデル・書き込み制限・料金・制限値、Durable Objects の料金・制限・強整合性の記述、D1 の制限・料金・レプリケーション機能、R2 の料金体系・egress無料・S3互換の実装状況区分、公式の storage-options 比較表。
- **一次情報はあるが定性的表現にとどまり、詳細裏取りが必要な項目**:
  - Durable Objects の「レートリミット」「ゲームのルーム状態」の具体的な実装パターンは、公式ページには用途名（multiplayer games, live notifications 等）の列挙はあるものの、実装手法の詳細は二次情報（オウンドブログ集約）由来。
  - R2 が「既存の S3 SDK が無改変で使える」という主張は二次情報のみで、公式 S3互換APIページでは機能差分（未実装機能あり）が明記されているため、**過度な一般化は避け、実際のツール互換性は個別検証が必要**と判断します。
  - S3 のエグレス料金（$0.09/GB、無料枠100GB等）との比較試算は二次情報（比較ブログ）由来であり、AWS公式では確認していません。**一次確認が必要**です。

## 関連ドキュメント

- [[20260802_kvs-nosql-cross-service-comparison]]（本書と対になる、Workers KV と他社KVS/NoSQLとの横断比較編）
- [[20260801_cloudflare-pages-deploy-considerations]]（Cloudflare Pages への frontend デプロイ検討）
- [[20260801_cloudflare-pages-local-deploy-practice]]（Cloudflare Pages 実践デプロイの作業記録）
- [[20260731_cloudflare-d1-fit-evaluation]]（D1 採用是非の検討。本書の D1 の技術特徴と合わせて参照可能）
