# CloudNativePG（CNPG）まとめ

作成日: 2026-08-16
出典: CloudNativePG公式ドキュメント・公式GitHubリポジトリ・CNCF/EDB公式発表（いずれも一次情報）。
下部の Sources を参照。

## 前提

- 本ノートは `docs_bevy_sample/20260816_kubernetes-supabase-self-hosting.md`（Kubernetes上での
  Supabaseセルフホスティング調査）で触れた「CNPG統合の要望」を深掘りするための調査記録。
  bevy_sample プロジェクトにおける本番バックエンドのホスティング方式検討の一環。
- 前回の調査で判明していたのは、Supabaseコミュニティが「セルフホストのPostgres部分をCNPGに
  任せたい」と要望しているが、Supabase公式は未対応（コミュニティの自力実装のみ）という事実。
  本ノートはCNPG自体（何ができるか・成熟度はどうか・Supabaseとの組み合わせ実例はあるか）を
  調べる。
- reading-notes リポジトリが見つからないため `docs_bevy_sample/` に保存。

## 1. CNPGとは何か（由来・ガバナンス・CNCF）

CloudNativePG（CNPG）は、Kubernetes上でPostgreSQLクラスタをデプロイ・運用するための
Kubernetes Operator。もともとは PostgreSQL のエンタープライズサポート企業 EDB
（EnterpriseDB）が 2019〜2020年頃に自社開発したもので、Apache License 2.0 でオープンソース
公開された。

その後、EDB は知的財産（IP）を「The CloudNativePG Authors」という中立的なコミュニティに
譲渡し、2025年1月21日に CNCF（Cloud Native Computing Foundation。Kubernetes 等も所属する
中立財団）の Sandbox（早期段階のプロジェクトを受け入れる区分）に採択された。EDB 自身が
「特定ベンダーが支配しない、オープンに統治された初の Postgres Operator」と位置づけている。
採用例として HSBC・GEICO Tech・Hitachi, Ltd. が公式サイトに挙げられている。

## 2. アーキテクチャの核心：`Cluster` というCRD、Patroni等の外部HAツールを使わない設計

CNPG の中心にあるのは `Cluster` という Custom Resource で、これが「1つの primary（書き込み
可能なインスタンス）＋任意個の hot standby replica」という PostgreSQL クラスタ 1 つ分を表す。

設計上の大きな特徴は、**Patroni・repmgr・Stolon のような外部の高可用性（HA）専用ツールに
一切依存せず、Kubernetes の API サーバー自体を状態管理の基盤として直接使う**こと。
StatefulSet も使わず、PVC（PersistentVolumeClaim）を Operator が直接管理する独自方式を
取っている。「immutable infrastructure」「宣言的設定」「マイクロサービスアーキテクチャ」
というクラウドネイティブなDevOps原則に基づいて設計されている、と公式が説明している。

## 3. Failoverとレプリケーション：クラスタ内は自動、クラスタ間は手動/GitOps

- **同一 Kubernetes クラスタ内**: 同期・非同期のストリーミングレプリケーションで
  primary→replica にデータを流し、primary障害時は「最もデータが新しいreplica」を自動的に
  promote する。読み書き用のサービス（`-rw` サービス）は、promote後に自動で新primaryを
  指すよう更新される。ここは完全に自動化されている。
- **クラスタ間（Replica Cluster）**: 別の Kubernetes クラスタ（別リージョン等）に置いた
  `Cluster` を、WALアーカイブ経由または直接のストリーミングレプリケーション経由で最初の
  クラスタから復旧・追従させる「Replica Cluster」という仕組みがある。ただし、**クラスタを
  横断したフェイルオーバー自体は自動化されておらず、手動操作またはGitOpsの運用フローで
  切り替える**、と公式ドキュメントに明記されている。

この「クラスタ内は自動、クラスタ間は手動」という制約は重要な発見である。
`docs_bevy_sample/20260816_kubernetes-supabase-self-hosting.md` §7 で触れた、Supabase
コミュニティの「アクティブ-アクティブレプリケーション（マルチリージョン等の分散デプロイ）が
未対応」という不満は、**CNPGを導入しても解消される保証はない**。CNPG自体もマルチリージョンの
自動フェイルオーバーは提供しておらず、Supabaseが抱える課題ではなくKubernetes Postgres
Operator全般が抱える一般的な限界に近い、と読める。

## 4. Connection Pooling（`Pooler` CRD）

CNPG は PgBouncer をラップした `Pooler` という別の Custom Resource でコネクションプーリング
を提供する。読み書き用・読み取り専用用を別々の `Pooler`（`-rw` / `-ro`）として立てるのが
一般的な構成（§7 の実例でも確認）。

## 5. Backup/PITR：in-tree方式からプラグイン方式（CNPG-I）への移行

CNPG のバックアップ機能は設計が一度変わっている。

- **旧方式（in-tree）**: `Cluster` リソース内の `.spec.backup.barmanObjectStore` に
  オブジェクトストレージの設定を直接書き込む、Operator組み込みの方式。
- **新方式（プラグイン）**: CNPG 1.26 以降で導入されたプラグイン機構（リポジトリ名
  `cloudnative-pg/plugin-barman-cloud` が示す通り「CNPG-I」と呼ばれるプラグインAPI）上に、
  Barman Cloud Plugin という形でバックアップ機能を切り出した。旧方式で作成済みのバックアップ
  とは互換性が保たれる。

Barman Cloud Plugin は Amazon S3・Google Cloud Storage・Azure Blob Storage の3大クラウドの
オブジェクトストレージに対応し、加えて MinIO・Azurite・fake-gcs-server といったS3互換／
シミュレータもテスト済み。WALのリアルタイムアーカイブと、オンデマンド／スケジュール実行の
物理バックアップ（ベースバックアップ）の両方に対応し、これらを組み合わせて PITR
（任意時点への復元）を実現する。

## 6. バージョン・サポートポリシー

取得日（2026-08-16）時点でドキュメントの最新版として確認できたのは 1.30 系
（`cloudnative-pg.io/docs/1.30/`）。マイナーリリースは「おおよそ2か月ごと」のペースで出され、
サポート期間は「そのマイナーバージョンのリリースから、N+1マイナーバージョンのリリース後
3か月まで」の合計およそ5か月間。セキュリティ修正・バグ修正はサポート対象の全リリースへ
バックポートされる。

参考までに 1.26/1.27/1.28 世代では PostgreSQL 13〜18、Kubernetes 1.31〜1.34 あたりを
対応範囲としていた（バージョンごとに対応範囲は前後にずれるため、実際に採用するときは
利用時点の最新の対応表を必ず確認すること）。

## 7. Supabaseとの実際の組み合わせ例：`voltade/cnpg-supabase`

CNPG は Postgres 全般を管理する汎用 Operator であり、Supabase 固有の拡張機能
（`pg_net`、`vault`、`pgjwt` 等）は同梱していない。そこで、CNPG自身が配布する公式ベースイメージ
`ghcr.io/cloudnative-pg/postgresql`（CNPGが直接操作できるよう調整されたPostgresイメージ）の上に、
Supabase の Postgres イメージが持つ拡張機能を追加で載せ直す、というコミュニティ実装が
`voltade/cnpg-supabase`（GitHub、Apache-2.0、15 stars、最終更新 2026-02-03）である。

- **Dockerfile**: `postgresql:17.6-minimal-trixie`（CNPG公式イメージ）をベースに、
  Supabase由来の拡張（`pg_net` 0.14.0、`pg_safeupdate` 1.5、`vault` 0.3.1、`pgjwt`、
  `pgsql_http`、`pg_graphql`、`wrappers`、`supautils`）と、一般的な運用向け拡張
  （`pg_cron`、`timescaledb`、`pgaudit`、`pg-failover-slots`、`pg_stat_kcache`、
  `pg_stat_statements`、`auto_explain`、`libsodium` 1.0.20）をビルドし直して追加している。
- **Helm chart（`charts/cnpg-cluster`）**: 具体的な `Cluster` の構成例として、
  `instances: 3`（primary 1 + replica 2 のHA構成）、S3（`ap-southeast-1`リージョンの
  `cnpg-backups` バケット）へのスケジュールバックアップ、読み書き／読み取り専用の
  `Pooler` を1台ずつ、`app` という業務用ロール（login・createrole・bypassrls権限）を
  定義したテンプレート一式（`cnpg-cluster.yaml` / `cnpg-object-store.yaml` /
  `cnpg-pooler-ro.yaml` / `cnpg-pooler-rw.yaml` / `cnpg-scheduled-backup.yaml` /
  `superuser-secret.yaml`）を含む。
- **成熟度**: リポジトリに README が存在せず、スター数も15と小規模。ドキュメント化された
  利用手順は無く、ソースコード（Dockerfile・Helm chart）を直接読んで使い方を推測する必要が
  ある状態。個人〜小規模チームによる実験的プロジェクトの域を出ていないと判断できる。

## 8. 位置づけの整理：Supabase公式Operatorとの関係

`docs_bevy_sample/20260816_kubernetes-supabase-self-hosting.md` §3 で確認した Supabase
公式のコミュニティOperator（`core.supabase.io/v1alpha1`）は、Postgres 管理を独自の
`SingleDatabase` CRDで行っており、CNPGの `Cluster` CRDとは全くの別実装である。つまり、
CNPGを使ってSupabaseをセルフホストしたい場合、公式Operatorの `SingleDatabase` は使わず、
`voltade/cnpg-supabase` のような**独自のCNPG `Cluster` を自分で用意し、Supabaseの他コンポーネント
（Auth/GoTrue・PostgREST・Storage・Realtime等）だけを別途デプロイして繋ぎ込む**という、
公式Operatorの外側での自作構成になる。「CNPG統合」というのは、既存の仕組みに設定を1つ足すような
話ではなく、Postgres層をまるごと別実装に置き換える規模の話である。

## 読み方の注意

- §1〜§6（CNPG本体の仕組み・ガバナンス・バージョン）は公式ドキュメント（`cloudnative-pg.io`）
  および公式GitHubリポジトリのREADMEを直接確認した一次情報。
- §7 の `voltade/cnpg-supabase` は GitHub API・raw ファイルを直接確認した一次情報だが、
  この実装自体がコミュニティの非公式プロジェクトである点に注意。Supabase公式・CNPG公式いずれの
  推奨手順でもない。
- §3 の「クラスタ間フェイルオーバーは手動/GitOps」という記述は、CNPGの `Cluster`/Replica
  Cluster に関する公式アーキテクチャページの記載に基づく。この制約が実際にどの程度の運用負荷に
  なるかは、本ノートでは検証していない（推測ではなく公式記載の事実だが、影響範囲の評価は
  行っていないという意味）。
- §6 の対応PostgreSQL/Kubernetesバージョン範囲は、取得時にたまたま参照したドキュメントの版
  （1.26系）に基づく数値であり、取得日時点の最新版（1.30系）の対応表そのものは未確認。
  実際に採用する際は `cloudnative-pg.io/docs/<最新版>/supported_releases/` を必ず確認すること。

## Sources

- [CloudNativePG - PostgreSQL Operator for Kubernetes（公式トップページ）](https://cloudnative-pg.io/)
- [CloudNativePG Documentation (1.30)](https://cloudnative-pg.io/docs/1.30/)
- [Architecture — CloudNativePG](https://cloudnative-pg.io/docs/1.30/architecture/)
- [Supported releases — CloudNativePG (1.26)](https://cloudnative-pg.io/docs/1.26/supported_releases/)
- [Barman Cloud Plugin | Barman Cloud CNPG-I plugin](https://cloudnative-pg.io/plugin-barman-cloud/docs/intro/)
- [GitHub - cloudnative-pg/cloudnative-pg](https://github.com/cloudnative-pg/cloudnative-pg)
- [GitHub - cloudnative-pg/plugin-barman-cloud](https://github.com/cloudnative-pg/plugin-barman-cloud)
- [CloudNativePG Officially Joins the CNCF Sandbox — EDB Blog](https://www.enterprisedb.com/blog/cloudnativepg-officially-joins-cncf-sandbox-milestone-cloud-native-postgresql)
- [CloudNativePG | CNCF](https://www.cncf.io/projects/cloudnativepg/)
- [GitHub - voltade/cnpg-supabase](https://github.com/voltade/cnpg-supabase)
- [Support for Kubernetes Postgres Operators (CNPG) · supabase · Discussion #31147](https://github.com/orgs/supabase/discussions/31147)

## 関連ドキュメント

- `docs_bevy_sample/20260816_kubernetes-supabase-self-hosting.md`（本ノートの調査の起点。
  §7でCNPG統合要望に触れている）
