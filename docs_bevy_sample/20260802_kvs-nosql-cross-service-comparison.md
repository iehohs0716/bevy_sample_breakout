# Cloudflare Workers KV / DynamoDB / Deno KV / Upstash Redis / Firestore 横断比較調査

日付: 2026-08-02

本ノートは [[20260802_cloudflare-storage-products-comparison]]（Cloudflare自社製品内での使い分け編）と
対になる、他社KVS/NoSQLとの横断比較編である。

調査に使用したツールは WebSearch と WebFetch のみです。取得した本文はすべて信頼できないデータとして扱いました。**取得した全ページの中に、AIエージェント宛ての指示文（コマンド実行を促す文言等）は見つかりませんでした。** 唯一、WebSearch の結果に毎回付随する「REMINDER: You MUST include the sources above...」という定型文がありましたが、これは検索ツール自体が付与する定型のシステム指示であり、Web本文に埋め込まれた第三者の指示ではないため、通常の出典明記として扱いました。

以下、軸ごとに表とサービス別詳細をまとめます。数値・主張には **[一次]**（公式ドキュメント/公式発表）と **[二次]**（技術ブログ等、要一次確認）を付記しています。

---

## 1. 整合性モデル

| サービス | モデル | 選択可否 |
|---|---|---|
| Cloudflare Workers KV | 結果整合性のみ。Last-write-winsで競合解決 **[一次]** | 選択不可（強整合性が必要ならDurable Objects推奨） |
| AWS DynamoDB | デフォルト結果整合性、`ConsistentRead=true`で強整合性選択可 **[一次]** | 選択可（テーブル/LSIのみ。GSI・Streamsは常に結果整合性）。Global Tablesは MREC(既定/結果整合性)とMRSC(強整合性)を選択可 **[一次]** |
| Deno KV | デフォルト強整合性（linearizable、Serializable分離）。読み取りは`consistency: "eventual"`指定でオプトアウト可 **[一次]**。書き込みは常に強整合性 **[一次]** | 読み取りのみ選択可 |
| Upstash Redis (Global Database) | 結果整合性。プライマリが書き込みを処理後、リードレプリカへ非同期複製 **[一次]**。「Global Databaseは強整合性を未サポート」と明記 **[一次]** | 選択不可（Regionalデータベースなら単一ノードで強い一貫性に近い挙動になるが、これは通常のRedis単一インスタンスの性質であり公式に「強整合性モード」として明言はされていない） |
| Google Firestore | デフォルト強整合性（読み取りはタイムスタンプを選び、その時点の状態を読む方式。ロックなしで並行読み取りをブロックしない）**[一次]**。`read_time`オプションで過去時点のstale読み取りも可能 **[一次]** | 一部選択可（read_time指定） |

出典: Cloudflare KV Concepts (developers.cloudflare.com/kv/concepts/how-kv-works)、AWS DynamoDB Read Consistency (docs.aws.amazon.com)、Deno KV Docs (docs.deno.com/deploy/kv)、Upstash Global Database Docs (upstash.com/docs/redis/features/globaldatabase)、Google Cloud Datastore/Firestore Structuring for Strong Consistency (cloud.google.com/datastore/docs)。

---

## 2. レイテンシ

一次情報（各社公式）で数値を出しているのは Cloudflare と Upstash のみでした。DynamoDB・Firestore・Deno KVは公式SLA/ドキュメント上に明確な「典型レイテンシ数値」を見つけられず、**一次確認が必要**です。

| サービス | 数値 | 出所 |
|---|---|---|
| Cloudflare Workers KV | ホットキー（キャッシュ済み）500µs〜10ms。コールドリードは中央ストアからの取得のため相対的に遅い。書き込みの全世界伝播は最大60秒以上 | **[一次]** developers.cloudflare.com/kv |
| Upstash Redis (Global Database) | 同一リージョンからの読み取り <1ms、同一リージョンからの書き込み <5ms、大陸間の読み書きは99パーセンタイルで50ms以下（Upstash社内テスト） | **[一次]**（ただし自社ベンチマークである点に留意）upstash.com/docs/redis/features/globaldatabase |
| AWS DynamoDB | 公式ドキュメントに具体的なミリ秒単位のレイテンシ数値は明記されていない（「シングルディジットミリ秒」という表現がマーケティング資料にあるが、今回のドキュメント調査では厳密な一次数値は未確認） | **要一次確認** |
| Google Firestore | 「強整合性はリーダーとの通信のためレイテンシが高くなり得る」という定性的記述のみで、具体的数値は未確認 | **要一次確認** |
| Deno KV | 公式に具体的なミリ秒数値は見当たらず | **要一次確認** |

**二次情報（Denoの自社比較記事、要注意）**: Deno公式ブログ「Deno KV vs. Cloudflare Workers KV, Upstash Redis, AWS DynamoDB, and Google Firestore」（deno.com/blog/comparing-deno-kv）は、p99レイテンシとして以下を公表しています。
- 読み取り: Deno KV 90ms、Upstash Regional 139ms、DynamoDB 700ms、Firestore 699ms、Cloudflare KV 742ms
- 書き込み: Deno KV 166ms、Upstash Regional 279ms、DynamoDB 560ms、Firestore 826ms、Cloudflare KV 866ms

このブログはDeno社自身の発表なので「Deno KVの数値」としては一次情報ですが、**競合4社の数値はDeno社が独自に測定したものであり、各社自身の公式発表ではありません**。測定条件（リージョン、ペイロードサイズ、ネットワーク経路等）が非公開なため、比較記事として参考にはなるものの、競合各社の数値は「二次情報かつ利害関係者(競合)による測定」として扱い、一次確認（各社が自ら公表したベンチマークとの突合）が必要です。特にCloudflare KVの数値（p99で742〜866ms）は、Cloudflare公式のホットキー500µs〜10msという記述と大きく乖離しており、測定条件（コールドリード中心か、キャッシュ未ヒットのシナリオか等）の違いが疑われます。単純にこの記事の数値だけを鵜呑みにしないよう注意してください。

---

## 3. 書き込みスループットの制限

| サービス | 制限内容 |
|---|---|
| Cloudflare Workers KV | **同一キーへの書き込みは1回/秒**（Workers Binding API経由では「1キーあたり1回/秒、キー間では無制限」）。異なるキーへの書き込みは無料プランで1,000回/日、有料プランは無制限 **[一次]** developers.cloudflare.com/kv/platform/limits |
| AWS DynamoDB | オンデマンドモードは自動スケール（テーブル単位の上限設定も可能）。プロビジョンドモードはRCU/WCUを設定し、パーティション間で均等分割（例: 30WCU・3パーティションなら各10WCU）。パーティションのバースト上限は通常配分の最大1.5倍 **[一次/二次混在]**。1アイテムの最大サイズは400KB **[二次、要一次確認]** |
| Deno KV | 秒間の明示的な回数上限は公式ドキュメントに見当たらず、代わりに月間の読み取り/書き込み「ユニット」課金（後述）で律速される設計 **[一次だが数値上限は不明瞭]** |
| Upstash Redis | Pay-as-you-goプランは最大10,000コマンド/秒、Fixed 100GB/500GBプランは最大16,000コマンド/秒。上限超過時はレート制限（帯域幅超過はトラフィックブロック、ストレージ超過は書き込みブロック） **[一次]** upstash.com/docs/redis/overall/pricing |
| Google Firestore | ドキュメント単位の更新レート上限あり（「1ドキュメントあたり無制限には更新できない」）。正確な上限値は書き込みレート・競合・影響を受けるインデックスに依存し、公式ドキュメントも具体的な回数を明言していない。コレクション単位ではなくドキュメント単位の制限なので、多数の異なるドキュメントへの同時書き込みは可能 **[一次だが定量値は非公開]** |

---

## 4. 料金体系

すべて一次情報（公式料金ページ）に基づきます。

| サービス | 無料枠 | 読み取り | 書き込み | ストレージ | データ転送 |
|---|---|---|---|---|---|
| Cloudflare Workers KV | 読取10万/日、書込/削除/一覧各1,000/日、ストレージ1GB（Free） | $0.50 / 100万（月1,000万まで無料、有料プラン） | $5.00 / 100万（月100万まで無料） | $0.50 / GB・月（月1GBまで無料） | 課金なし |
| AWS DynamoDB (オンデマンド, US East) | 月25GBストレージ、25WCU/25RCU相当の無料枠、ストリーム読み取り250万件、送信1GB（初年度15GB） | 強整合性 $0.25/100万RRU、結果整合性 $0.125/100万RRU（4KB単位）※US East。トランザクション読み取りは2倍消費 | $1.25/100万WRU（1KB単位）。トランザクション書き込みは2倍消費 | Standard $0.25/GB・月、Standard-IA $0.10/GB・月 | 受信は無料。同一リージョン内送信は無料。リージョン間は課金あり |
| Deno KV | Free: ストレージ1GiB、読取45万ユニット/月(4KiB単位)、書込30万ユニット/月(1KiB単位) | 超過分 $1/100万ユニット（Proプラン） | 超過分 $2.5/100万ユニット（Proプラン） | Free 1GiB、Pro 5GiB | 個別記載なし（Deno Deployの帯域枠に含まれる模様、要確認） |
| Upstash Redis (Pay as you go) | 256MBデータ、50万コマンド/月（Freeプラン） | $0.20 / 10万コマンド（読み書き同額、コマンド課金は読み書き区別なし） | 同上 | 初回1GB無料、以降$0.25/GB | 月200GBまで無料、以降$0.03/GB |
| Upstash Redis (Fixed) | プランに含まれる固定枠 | コマンド課金なし（`commands are never metered`） | 同上 | プラン容量内に含む（例: 250MB=$10/月、1GB=$20/月、100GB=$800/月。追加リージョンは+$5〜+$400/月） | プラン内に含む |
| Google Firestore | 無料枠(1プロジェクト1つの無料DB): 読取5万/日、書込2万/日、削除2万/日、ストレージ1GiB、送信10GiB/月 | Google Cloud料金SKUページ参照（公式Firestore/Firebase料金ページは詳細単価を外部SKUページに委譲しており、今回の取得では正確な単価は「1インデックスエントリ最大1,000件で1読み取り」等の計算ルールのみ確認。二次情報では読取$0.03〜0.06/10万件、書込$0.09〜0.18/10万件という報告があるが**一次確認が必要** | 同上 | 二次情報で$0.15〜0.18/GB・月という報告あり**一次確認が必要** | 地域間送信は$0.01〜/GB（最初の10GiB/月は無料）|

**読み方の注意**: DynamoDBの「$0.6250 per million writes」という数値がWebFetch要約中に一度出ましたが、別の検索で確認した「2024年11月のオンデマンド料金50%値下げ後のUS East料金 $1.25/100万WRU」という数値と整合しないため、前者は古い料金や別条件の可能性があり採用していません。上表のUS East数値（$1.25 WRU、$0.25 strong RRU、$0.125 eventual RRU）は複数の検索結果で一致しているため、こちらを採用していますが、最終確認は `https://aws.amazon.com/dynamodb/pricing/on-demand/` の実ページ（料金計算機）で行うことを推奨します。

Firestoreの読み取り/書き込み単価は、firebase.google.com/docs/firestore/pricing 自体が「正確な単価はGoogle Cloud pricing（SKU）を参照せよ」と外部委譲しており、今回のWebFetchでは本文が長すぎて切り詰められ単価テーブルを直接確認できませんでした。**単価そのものは一次確認が必要**です。

---

## 5. スケーラビリティ

| サービス | ストレージ上限 | スループット上限 |
|---|---|---|
| Cloudflare Workers KV | アカウント/名前空間あたり無料1GB、有料は無制限（キーサイズ512B、値25MiB上限）**[一次]** | 読み取りは事実上無制限（エッジキャッシュ経由で「数千読み取り/秒/キー」も可能）。書き込みは1キー1回/秒が実質的なボトルネック **[一次]** |
| AWS DynamoDB | テーブルサイズは実質無制限（LSIを使わない場合、パーティション数は無制限にスケール）**[二次、要一次確認]**。LSI使用時はアイテムコレクション10GB上限、GSIは20個/テーブル、LSIは5個/テーブル **[二次]**。アイテム最大400KB **[二次]** | オンデマンドは自動スケール、テーブル単位の最大スループット上限設定も可能。プロビジョンドはRCU/WCU設定＋オートスケーリング。Query/Scanは1リクエストあたり最大1MBデータ **[二次]** |
| Deno KV | Free 1GiB、Pro 5GiB（Deno Deploy上）**[一次]**。バックエンドはFoundationDB **[一次]** | 明示的な秒間上限の記載は見当たらず、月間ユニット課金で律速 |
| Upstash Redis | Pay-as-you-go最大100GB、Fixedプランは250MB〜500GBの階層 **[一次]** | Pay-as-you-go最大10,000コマンド/秒、Fixed上位プラン16,000コマンド/秒 **[一次]** |
| Google Firestore | プロジェクトあたり最大100データベース。ドキュメント最大1MiB、フィールド値最大約1MiB-89byte **[一次]**。データベース全体の容量上限は今回未確認 | 秒間の全体スループット上限は明記されず、ドキュメント単位のホットスポット制約が実質的な律速要因 |

---

## 6. クエリ機能

| サービス | 機能 |
|---|---|
| Cloudflare Workers KV | 単純なキー取得のみ。`list`によるキーのプレフィックス絞り込みは可能だが、セカンダリインデックス・範囲検索・集計機能はなし **[一次]** |
| AWS DynamoDB | 主キー(パーティションキー+ソートキー)によるQuery、フルスキャン(Scan)に加え、GSI（別属性でのインデックス、最大20個/テーブル）・LSI（同一パーティションキー内の別ソート順、最大5個/テーブル）でセカンダリアクセスパターンに対応。範囲検索はソートキーで可能。集計は基本的にアプリ側かDynamoDB Streams+Lambda等の別経路が必要 **[二次、要一次確認]** |
| Deno KV | `list()`によるプレフィックス+レキシコグラフィック順の範囲検索、`getMany()`での一括取得。セカンダリインデックスは「同じデータを別キーでも保存する」手動パターンで実現（公式にセカンダリインデックス機構が用意されているわけではない）**[一次]** |
| Upstash Redis | Redis互換のデータ構造（String, Hash, Sorted Set, List, Stream等）による多様なアクセスパターンが可能。Upstash Redis Searchで全文検索にも対応 **[一次]** |
| Google Firestore | 単純フィールドの等価/範囲検索に加え、複合インデックス(Composite Index)による複数フィールドの範囲・不等号フィルタ、`orderBy`との組み合わせに対応。集計クエリ（count等）はインデックスをスキャンしてサーバー側で計算し、ドキュメント全読み取りより課金を抑えられる **[一次]** cloud.google.com/firestore（複合インデックス・集計クエリの公式ドキュメント） |

---

## 7. 運用面（フルマネージド度合い・配置）

| サービス | フルマネージド度合い | 配置 |
|---|---|---|
| Cloudflare Workers KV | フルマネージド。`wrangler.toml`での名前空間設定とバインドが必要。ローカル開発と本番の差異検証がやや複雑という二次情報あり **[二次]** | エッジ（Cloudflareグローバルネットワーク、約300拠点、単一のプライマリリージョンという概念がない）中央ストア+エッジキャッシュのハイブリッド **[一次/二次混在]** |
| AWS DynamoDB | フルマネージド。ただしAWSの設定・IAM周りは複雑になりがちという二次情報の指摘あり。ローカル開発にはDynamoDB Localの別途セットアップが必要 **[二次]** | リージョン配置（Global Tablesで複数リージョンにレプリケーション可能。二次情報では17リージョン展開との記載）**[二次、要一次確認]** |
| Deno KV | フルマネージド（Deno Deployにバックエンド統合、FoundationDBベース）。1行のコードで利用開始でき、APIキー管理不要という開発者体験の良さが強調される（Deno公式ブログ、自社評価につき割り引いて解釈） **[一次だが自己評価]** | プライマリリージョンは北バージニア(us-east4)で、ヨーロッパ・アジアにリードレプリカ。クロスリージョンレプリケーションは「現状非対応」という記載もあり、別ソースの「35リージョンでのグローバル読み取りレプリケーション」という記述とは整合性が取れておらず、**時期やプラン(Freeか有償か)による差の可能性があるため一次確認が必要** |
| Upstash Redis | フルマネージド・サーバーレス。Regional/Globalの2モード選択可 **[一次]** | Regionalは単一AWSリージョン、GlobalはAWS14リージョン+GCP4リージョン(計18ロケーション)にレプリケーション、単一プライマリ+複数リードレプリカ **[一次]** |
| Google Firestore | フルマネージド。プロジェクトごとに（無料枠は）1データベースというシンプルな接続体験 **[一次/二次]** | リージョン配置（マルチリージョン構成も選択可能。二次情報で23リージョン展開との記載）**[二次、要一次確認]** |

---

## 8. 典型的なユースケース

| サービス | 公式に言及される/コミュニティでよく使われる用途 |
|---|---|
| Cloudflare Workers KV | 設定データ・機能フラグ・A/Bテスト用のパーソナライゼーションデータ、セッション/認証情報・APIキーの保管、allow-list/deny-list、読み取り頻度が高く更新頻度が低いデータのキャッシュ **[一次]** developers.cloudflare.com/kv |
| AWS DynamoDB | ゲーム（大規模同時接続、ショッピングカート・在庫・顧客プロファイル）、金融サービス（取引台帳、トークン生成、ACIDトランザクションが必要なワークロード）、メディア/エンタメ（複数リージョンでの低レイテンシ配信）、モバイル/Web/adtech/IoT全般で「主キーによる高速アクセス」が必要な用途 **[一次]** aws.amazon.com/dynamodb、AWS Database Blog |
| Deno KV | リアルタイムアプリ（通知、ニュースフィード、マルチプレイヤーゲーム）、ユーザーUI設定の保存、チャットメッセージの`watch()`による監視、条件付き書き込み（アトミックトランザクション）**[一次]** deno.com/blog |
| Upstash Redis | DBクエリ/APIレスポンスのキャッシュ、レート制限（Upstash Ratelimit SDK）、ジョブキュー（FIFO・遅延・優先度付き）、サーバーレス関数間のセッション管理、全文検索、AIエージェントの短期/長期メモリ・LLMレスポンスキャッシュ・チャット履歴 **[一次]** upstash.com/docs/redis/overall/usecases |
| Google Firestore | モバイル/Webアプリのバックエンド（リアルタイム同期、オフライン対応）、チャット・メッセージングアプリ、共同編集アプリ、SNS/ニュースのライブフィード、同期が必要なマルチプレイヤーゲーム、小売・メディア・通信・IoTのリファレンスアーキテクチャ **[一次]** firebase.google.com/docs/firestore |

---

## 読み方の注意（まとめ）

1. **Denoの自社比較ブログ（deno.com/blog/comparing-deno-kv）は要注意情報源**です。Deno KV自身の数値は一次情報として扱えますが、競合4社（Cloudflare KV, DynamoDB, Upstash, Firestore）のレイテンシ・リージョン数はDeno社が独自測定・独自集計したものであり、各社の公式発表ではありません。特にCloudflare KVのp99レイテンシ（742〜866ms）はCloudflare公式ドキュメントの「ホットキー500µs〜10ms」という記述と大きく乖離しており、測定条件の違い（コールドリード中心の可能性等）が疑われます。数値を引用する際は必ず「Deno社調べ」と明記してください。
2. DynamoDBのオンデマンド料金は複数の二次情報で「2024年11月に50%値下げ」があったとされ、WebFetchで一度取得した「$0.625/100万書き込み」という数値と、別途確認した「$1.25/100万WRU（US East）」が食い違いました。本ノートでは後者（複数ソースで一致）を採用していますが、最終確認は公式料金計算機で行ってください。
3. Firestoreの読み取り/書き込み単価は公式ページ自体がSKUページへの参照に留めており、今回のWebFetchでは正確な単価テーブルを直接確認できませんでした。二次情報の数値（読取$0.03〜0.06/10万件等）は**一次確認が必要**です。
4. DynamoDBのリージョン数（17）、Firestoreのリージョン数（23）、Upstashのリージョン構成の一部細目は、Deno比較ブログや技術ブログなど二次情報由来のため、AWS/GCPの公式リージョン一覧ページでの裏取りを推奨します。
5. Deno KVのリージョン構成について、「北バージニアがプライマリでクロスリージョンレプリケーション非対応」という記述と「35リージョンでグローバル読み取りレプリケーション」という記述が両方見つかり、内容が矛盾しています。ドキュメントの更新時期やDeno Deployのプラン（Free/Pro/Enterprise）による差の可能性があるため、契約前提で使うなら公式ドキュメントの最新版を直接確認してください。
6. 不審な指示文の混入は確認されませんでした。

## Sources

- [How KV works — Cloudflare Workers KV docs](https://developers.cloudflare.com/kv/concepts/how-kv-works/)
- [Limits — Cloudflare Workers KV docs](https://developers.cloudflare.com/kv/platform/limits/)
- [Pricing — Cloudflare Workers KV docs](https://developers.cloudflare.com/kv/platform/pricing/)
- [Choosing a data or storage product — Cloudflare Workers docs](https://developers.cloudflare.com/workers/platform/storage-options/)
- [DynamoDB read consistency — Amazon DynamoDB Developer Guide](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/HowItWorks.ReadConsistency.html)
- [Amazon DynamoDB Pricing](https://aws.amazon.com/dynamodb/pricing/)
- [Amazon DynamoDB On-Demand Pricing](https://aws.amazon.com/dynamodb/pricing/on-demand/)
- [Quotas in Amazon DynamoDB](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/ServiceQuotas.html)
- [Constraints in Amazon DynamoDB](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/Constraints.html)
- [What is Amazon DynamoDB?](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/Introduction.html)
- [Amazon DynamoDB use cases for media and entertainment customers — AWS Database Blog](https://aws.amazon.com/blogs/database/amazon-dynamodb-use-cases-for-media-and-entertainment-customers/)
- [Common financial services use cases for Amazon DynamoDB — AWS Database Blog](https://aws.amazon.com/blogs/database/common-financial-services-use-cases-for-amazon-dynamodb/)
- [KV Quick Start — Deno Docs](https://docs.deno.com/deploy/kv/)
- [Deno KV — Deno Docs (reference)](https://docs.deno.com/deploy/reference/deno-kv/)
- [Announcing Deno KV — Deno blog](https://deno.com/blog/kv)
- [Deno KV internals: building a database for the modern web — Deno blog](https://deno.com/blog/building-deno-kv)
- [Deno KV vs. Cloudflare Workers KV, Upstash Redis, AWS DynamoDB, and Google Firestore — Deno blog](https://deno.com/blog/comparing-deno-kv)
- [Pricing & Limits — Upstash Documentation](https://upstash.com/docs/redis/overall/pricing)
- [Global Database — Upstash Documentation](https://upstash.com/docs/redis/features/globaldatabase)
- [Use Cases — Upstash Documentation](https://upstash.com/docs/redis/overall/usecases)
- [Structuring Data for Strong Consistency — Google Cloud Datastore Docs](https://cloud.google.com/datastore/docs/concepts/structuring_for_strong_consistency)
- [Balancing Strong and Eventual Consistency with Datastore — Google Cloud Docs](https://docs.cloud.google.com/datastore/docs/articles/balancing-strong-and-eventual-consistency-with-google-cloud-datastore)
- [Quotas and limits — Firestore (Google Cloud Docs)](https://docs.cloud.google.com/firestore/quotas)
- [Firestore pricing — Google Cloud](https://cloud.google.com/firestore/pricing)
- [Pricing — Firestore | Firebase](https://firebase.google.com/docs/firestore/pricing)
- [Query with range and inequality filters on multiple fields — Firestore Docs](https://docs.cloud.google.com/firestore/native/docs/query-data/multiple-range-fields)
- [Summarize data with aggregation queries — Firestore | Firebase](https://firebase.google.com/docs/firestore/query-data/aggregation-queries)
- [Understand real-time queries at scale — Firestore | Firebase](https://firebase.google.com/docs/firestore/real-time_queries_at_scale)

（二次情報として参照し、本文中で個別に「要一次確認」と付記した記事: dynobase.dev/dynamodb-limits、dynobase.dev/dynamodb-read-consistency、jayendrapatil.com のDynamoDB/Firestore解説、airbyte.com のDynamoDB/Firestore料金解説、cloudburn.io のDynamoDB料金解説、oreateai.com のDynamoDB書き込み単価解説）

## 関連ドキュメント

- [[20260802_cloudflare-storage-products-comparison]]（本書と対になる、Cloudflare自社製品内での使い分け編）

関連: プロジェクトのベンダーロックイン回避方針（Web公開/UGC設計はSupabase等を自前API層で抽象化し、フロントから直叩きしない）の検討材料としても、本ノートの整合性モデル・料金・ポータビリティに関する比較情報は使える。
