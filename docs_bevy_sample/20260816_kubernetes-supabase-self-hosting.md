# Kubernetes上でのSupabaseセルフホスティング まとめ

作成日: 2026-08-16
出典: Supabase公式ドキュメント・GitHub公式リポジトリ（`supabase-community/supabase-kubernetes`
のREADME・values.yaml・GitHub API経由のコミット/リリース履歴）が一次情報。UpCloud公式ガイドは
二次情報として実運用上の一般論のみ補助的に参照。下部の Sources を参照。

## 前提

- 本ノートは bevy_sample プロジェクト（Bevy 製ブロック崩しを WASM 化し React フロントから
  起動するサンプル。Web 公開・UGC 基盤を構築中）における調査記録。
- 本プロジェクトの `doc_arch/backend.md` §1 では、本番バックエンドとして「Supabase（Auth・
  ユーザー情報）＋ DynamoDB（ゲームデータ）」の採用が既に決定済み。ただし `doc_arch/overview.md`
  の未決事項一覧に「Cloudflare Workers から Postgres（Supabase／将来の Neon）への接続方式」が
  残っており、**Supabase を本番でどう動かすか（Supabase Cloud のマネージドサービスを使うか、
  自前でセルフホストするか）自体はまだ決定していない**。ローカル開発は `supabase start`
  （Supabase CLI・Docker Compose ベース）で完結している（`doc_arch/deploy.md` §1）。
- 本ノートはこの未決事項に対する選択肢の一つ「Kubernetes 上でのセルフホスト」について、
  現時点（2026-08-16）の最新状況を調査したものであり、**採用を決定した記録ではない**。
  doc_arch を更新するかどうかは、この調査結果を踏まえて別途ユーザーと相談の上で判断する。
- reading-notes リポジトリがこの作業マシン上に見つからなかったため、本プロジェクトの設計議論
  ログの置き場である `docs_bevy_sample/` に、同フォルダの命名規則
  （`YYYYMMDD_kebab-case.md`）で保存している。
- 関連ノート:
  - `docs_bevy_sample/20260816_supabase-api-gateway-kong-to-envoy.md`
    （Supabase self-hosted 構成のデフォルト API ゲートウェイが Kong から Envoy に切り替わった
    経緯。本ノート §4 で、この移行が Kubernetes 側の実装にもそのまま反映されていることを確認する）
  - `docs_bevy_sample/20260816_gotrue-overview-and-supabase-cloud-pricing.md`
    （GoTrue／Supabase Auth の由来と Supabase Cloud の課金体系）

## 1. 公式の立場：Docker Composeが第一選択、Kubernetesはコミュニティ管轄

Supabase 公式のセルフホスティングドキュメントは、Docker Compose を明確に推奨している。

> "The fastest and recommended way to self-host Supabase is to use Docker."

Kubernetes への対応にも触れているが、それは「Helm charts を使った Kubernetes へのデプロイ」
という形の言及であり、実体は公式リポジトリ配下の一部ではなく、後述するコミュニティ主導の
別リポジトリを指している。セルフホスト版全般（Docker Compose・Kubernetes いずれも共通）の
制約として、次の機能が利用できない点も明記されている。

- ブランチング（Supabase Cloud のプレビュー環境機能）
- マネージドバックアップと PITR（Point-in-Time Recovery）
- ベクトルバケット
- 高度なメトリクス
- 複数プロジェクト・複数組織のサポート

つまり「Kubernetes で動かせば自動的にマネージドバックアップ相当が手に入る」わけではなく、
バックアップ・PITR は Kubernetes 上でも自分で構築する必要がある。

## 2. デファクトスタンダードのリポジトリ: `supabase-community/supabase-kubernetes`

Kubernetes 上でのセルフホストにおいて、現時点でコミュニティのデファクトスタンダードと言える
実装は `supabase-community/supabase-kubernetes`（GitHub、802 stars・287 forks、Apache-2.0
ライセンス）である。README には次の一文が明記されている。

> "This project is supported by the community and not officially supported by Supabase."

前提条件は Kubernetes 1.28 以上・Helm 3.x 以上・`kubectl` 設定済み。Go 言語で書かれた
Kubebuilder ベースのプロジェクト構成（`api/` `cmd/` `internal/` `config/` ディレクトリ、
`PROJECT` ファイル）を取っており、単なる Helm chart 集ではなく Kubernetes Operator の
実装を伴う。

## 3. 二つの導入方式：従来のHelmチャート vs 新しいOperator

このリポジトリは、成熟度が異なる 2 つの導入方式を並行して提供している。

| 方式 | 内容 | 最新バージョン・公開日 |
|---|---|---|
| 従来型 Helm chart（`charts/supabase`） | 各コンポーネントを個別の Deployment として並べる、比較的枯れた方式。12 個の Pod をデプロイする | `supabase-0.7.2`（2026-07-23 公開） |
| Kubernetes Operator（`charts/supabase-operator` + `charts/supabase-project`） | `core.supabase.io/v1alpha1` の Custom Resource（`Project` / `SingleDatabase` / `Function` / `Migration`）でSupabaseインスタンスを宣言的に管理する新方式 | `supabase-operator-0.1.3`（2026-07-23 公開）。初回リリース `supabase-operator-0.1.0` は 2026-07-15 |

Operator 方式は README 上で明確に「開発初期段階であり、API が変更される可能性がある」
（early stage of development, API may change）と注記されている。バージョン番号も
`v1alpha1` / `0.1.x` であり、本番導入には慎重な検討が必要な段階と判断できる。

Operator 方式のカスタムリソース例（`config/samples/project.yaml` および
`config/samples/singledatabase.yaml` から一次情報として原文のまま抜粋）は次の通り。
`auth.oauth.google` フィールドがあり、Google OAuth の Client ID・Secret（`secretRef` で
Kubernetes Secret を参照）をこの CR 内で直接宣言できることが分かる。

```yaml
apiVersion: core.supabase.io/v1alpha1
kind: SingleDatabase
metadata:
  name: supabase
spec:
  storage:
    accessModes:
      - ReadWriteOnce
    size: 20Gi
    # deletionPolicy: Delete ## Or Retain
```

```yaml
apiVersion: core.supabase.io/v1alpha1
kind: Project
metadata:
  name: supabase
spec:
  # jwtExpSec: 1800

  http:
    protocol: "http"
    hostname: "api.supabase.local"

  databaseRef:
    kind: SingleDatabase
    name: supabase

  envoy:
    enable: true
    # replicas: 2
    # service:
    #   type: LoadBalancer

  meta:
    enable: true
  rest:
    enable: true

  storage:
    enable: true
    storage:
      accessModes:
        - ReadWriteOnce
      size: 5Gi

  realtime:
    enable: true

  functions:
    enable: true
    verifyJwt: false

  studio:
    enable: true
    orgName: "Default Organization"
    projName: "Default Project"
    storage:
      accessModes:
        - ReadWriteOnce
      size: 1Gi

  auth:
    enable: true
    siteUrl: "https://myapp.supabase.com"
    # smtp:
    #   ...
    # oauth:
    #   google:
    #     enable: true
    #     clientId: "google-client-id"
    #     secretRef:
    #       name: "oauth-google-secret"
    #       key: "secret"
```

## 4. ゲートウェイの選択がHelmチャートとOperatorで分かれている（Kong vs Envoy）

`charts/supabase`（従来型 Helm chart）の `values.yaml` を確認すると、API ゲートウェイの
コンポーネントは今も `kong/kong:3.9.1` である。一方、Operator 方式の `Project` CRD には
`kong` フィールドは存在せず、上記サンプルの通り `envoy` フィールドで API ゲートウェイを
有効化する構造になっている。

つまり、`docs_bevy_sample/20260816_supabase-api-gateway-kong-to-envoy.md` で確認した
「Docker Compose 版セルフホストのデフォルトゲートウェイが 2026-08-09 の週から Kong から
Envoy に切り替わった」という流れは、Kubernetes 版にもそのまま反映されている。ただし
反映の仕方は Docker Compose とは異なり、「同じ Helm chart 内で Kong を Envoy に置き換えた」
のではなく、「Kong を使う枯れた旧方式（Helm chart）」と「Envoy を前提とする新方式
（Operator）」という**2つの別方式として並存**する形になっている。今から新規に Kubernetes
上でセルフホストするなら、Operator 方式（Envoy 前提）の方が公式の今後の方向性と一致する。
ただし Operator 自体が `v0.1.x` の早期段階であることとのトレードオフになる。

## 5. コンポーネントとイメージバージョン（従来Helmチャート、2026-07-23時点）

従来型 Helm chart（`supabase-0.7.2`）の `values.yaml` に記載された各コンポーネントの
コンテナイメージは次の通り。

| コンポーネント | イメージ | タグ |
|---|---|---|
| PostgreSQL（DB） | `supabase/postgres` | `17.6.1.136` |
| 認証（GoTrue/Supabase Auth） | `supabase/gotrue` | `v2.189.0` |
| REST API（PostgREST） | `postgrest/postgrest` | `v14.12` |
| リアルタイム（Realtime） | `supabase/realtime` | `v2.102.3` |
| ストレージ（Storage API） | `supabase/storage-api` | `v1.60.4` |
| メタデータ（Postgres Meta） | `supabase/postgres-meta` | `v0.96.6` |
| API ゲートウェイ（Kong） | `kong/kong` | `3.9.1` |
| 管理画面（Studio） | `supabase/studio` | `2026.07.07-sha-a6a04f2` |
| 画像変換（Imgproxy） | `darthsim/imgproxy` | `v3.30.1` |
| Edge Functions | `supabase/edge-runtime` | `v1.74.0` |
| S3互換ストレージ（MinIO） | `cgr.dev/chainguard/minio` | `latest` |
| ログ分析（Logflare） | `supabase/logflare` | `1.43.1` |
| ログパイプライン（Vector） | `timberio/vector` | `0.53.0-alpine` |

PostgreSQL は 2026-07-14 公開の `supabase-0.7.0` で v15 系から v17 系へアップグレードされた
（同リリースで Realtime の暗号化キー生成機能も追加）。

## 6. 開発状況・アクティビティ

`supabase-community/supabase-kubernetes` は活発に開発が継続している。直近のコミット
（`e88d61f`、2026-08-05）は「Realtime リクエストを default tenant にルーティングする」
Envoy 側の修正であり、Envoy 移行に追随した実装調整が継続的に入っていることが分かる。
すべてのコミットは単一のメンテナー（Luiz Felipe Machado 氏）によるもので、コミュニティ
主導としては継続的なコミット活動がある一方、メンテナーの層は薄い可能性がある（この点は
コミット履歴からの推測であり、コントリビューター体制について明示的な記載は確認していない）。

## 7. 未解決の課題（GitHub Discussionより）

Supabase 公式の GitHub Discussion「Self-hosting: What's working (and what's not)?」
（#39820、2025-10-23開始、最新コメント2026-04-29）では、Kubernetes関連で次の要望・課題が
挙がっている。

- **CloudNativePG（CNPG）等の Kubernetes Postgres Operator との統合**：jniclas 氏が最優先
  課題として言及。ただし別ディスカッション（#31147「Support for Kubernetes Postgres
  Operators (CNPG)」）を確認したところ、**Supabase 公式スタッフからの直接回答は無く、
  完全にコミュニティの自力実装に委ねられている**状態。コミュニティの実装パターンは
  `supabase/postgres` イメージに `barman`（PITR用バックアップツール）を追加し、init
  container でマイグレーションを実行する、というもの（Towerful 氏・mdluo 氏・
  AntonOfTheWoods 氏らが2024年12月〜2025年4月の間に公開）。§1 で述べた「セルフホストには
  マネージドバックアップ・PITR が無い」というギャップを埋める試みだが、公式サポートでは
  ない。
- **クラウド非依存の最新テンプレート**：odicis 氏（スタートアップ運営者）が「Up-to-date,
  cloud-agnostic Kubernetes templates」の必要性を指摘。
- **Changelog の不足**：バージョン間の互換性情報が不足しているとの指摘。
- **アクティブ-アクティブレプリケーション**：分散デプロイ（マルチリージョン等）が未対応。
- Supabase 公式スタッフ（aantti氏、Self-hosting 専任）は、AWS Aurora 対応の試験的ブランチや
  `./docker/tests` へのテストスクリプト追加など Docker Compose 側の改善を進めているとの
  コメントがあるが、Kubernetes（Operator含む）への公式コミットには言及していない。

## 8. 実運用にあたっての一般的な注意点

以下は UpCloud 公式ガイドから抽出した一般論であり、Supabase 自体の一次情報ではなく、
Kubernetes 運用に関する二次的な実践知として参照する。

- Postgres のデータ永続化には Kubernetes の PersistentVolumeClaim（PVC）が前提になる。
- 最小構成の起点として「2 vCPU / 4GB メモリの worker node 1 台」という具体的なサイジング例
  が挙げられている（ただしこれは UpCloud の特定プラン紹介文脈であり、汎用的な推奨値として
  裏付けが取れているわけではない）。
- Kubernetes のセルフヒーリング（Pod自動復旧）がメリットとして紹介されているが、これは
  Kubernetes 一般の特性であり Supabase 固有の話ではない。

## 9. 補足: docker-compose.ymlをそのままクラウドにデプロイする手段（Kubernetes以外の選択肢）

§1で確認した「セルフホストの第一選択はDocker Compose」という公式の立場を踏まえると、
「Kubernetesまで行かず、既存の `docker-compose.yml` をそのまま何らかのクラウドサービスに
持ち込めないか」という選択肢も比較対象になる。調査した結果、大きく2系統に分かれる。

### マネージドクラウドがcompose.ymlをネイティブに受け付ける場合

| サービス | 現状 |
|---|---|
| Azure Container Apps（`az containerapp compose create`） | 現時点（2026-08-16確認）でGA（提供終了していない）。compose内の各serviceを個別のContainer Appに変換してデプロイする |
| AWS ECS（Docker Compose CLI統合、ECS context） | **2023年11月に廃止済み**。かつては`docker compose up`でそのままECSにデプロイできたが、現在は使えない選択肢 |
| Google Cloud Run | compose.ymlを直接インポートする機能自体が無く対象外 |

Azure Container Apps は現存する数少ない「マネージドクラウドがcompose.ymlをそのまま読む」
サービスだが、Supabaseのような十数コンテナ構成・複雑なヘルスチェック・ボリューム共有を持つ
compose定義を、Container Apps側の変換ロジックがどこまで無修正で扱えるかは未検証（変換した
結果、個別のマネージドNW/ボリュームの流儀に合わせて調整が要る可能性がある）。

### VPSに自分で立てるセルフホストPaaS（実務上の主流）

**CoolifyやDokploy**が代表的。これらは「Herokuを自分のVPS（Hetzner・DigitalOcean等）上で
動かす」ようなOSSのセルフホストPaaSで、`docker-compose.yml`をほぼそのまま貼り付けて
デプロイ・リバースプロキシ・SSL証明書の発行まで面倒を見てくれる。

Supabaseのセルフホストという文脈では、**実際に使われているのはこちら側**である。Coolifyも
Dokployも、Supabaseのdocker-compose構成をテンプレートとして最初から用意しており、
「Supabase self-hosted on Coolify」「Supabase self-hosted on Dokploy」という実例記事が
複数存在する。ただしCoolifyはcompose経由のデプロイではゼロダウンタイムデプロイに対応して
いない（Dockerfile/Nixpacks/単一イメージデプロイのみ対応）という制約がある、との言及があった
（二次情報。一次ソースでの確認はしていない）。

つまり「compose.ymlをそのまま使う」という軸で見た場合の現実的な選択肢の序列は、
**Coolify/Dokploy（VPS上のセルフホストPaaS）＞ Azure Container Apps（マネージドクラウドの
compose直接インポート）＞ Kubernetes（Helm chart/Operatorへの変換が別途必要）** という順に
「compose.ymlからの距離が近い」。Kubernetesを選ぶ理由は、compose.ymlをそのまま使う便利さでは
なく、§2〜§8で見たマルチノードでの高可用性・宣言的なCRDによる管理・将来のCNPG等との統合余地
にある。

## 読み方の注意

- §2〜§7 の一次情報は、GitHub の Web ページ（README・values.yaml）と GitHub REST API
  （`/releases` `/commits` エンドポイント）を実際に取得して確認した。
- **裏取りの過程で1件、二次要約の誤りを検出した**: GitHub リリース一覧ページを WebFetch で
  取得・要約した最初の結果では最新リリース日が「2024年7月23日」と報告されたが、GitHub API
  （`/repos/.../releases`）を直接取得して突き合わせたところ実際は「2026年7月23日」だった。
  ページ内テキストのAI要約は年号を誤って読み取ることがあるため、日付が重要な判断根拠になる
  場合はAPIやraw取得で再確認するのが安全、という教訓として明記しておく。
- Operator 方式（`core.supabase.io/v1alpha1`）は README 自身が「早期段階でAPIが変わりうる」
  と明記している段階のソフトウェアであり、本番採用の判断材料にする場合はこの成熟度を
  踏まえる必要がある。
- §7 のコミュニティ Discussion 内容は、投稿者個人の実装パターン紹介であり Supabase 公式の
  推奨手順ではない。CNPG統合を採用する場合は、紹介されている実装を出発点として自己責任で
  検証する前提になる。
- §8 は二次情報（UpCloud公式ガイド）であり、数値（サイジング例）は該当ベンダーの文脈に
  依存する可能性がある。汎用的な推奨値として引用する場合は一次情報での裏付けが必要。
- 本ノートが調査したのはあくまで「選択肢としての現状」であり、`doc_arch` 側の本番ホスティング
  方針（Supabase Cloud か自前セルフホストか）はまだ決定されていない。決定する場合は
  `doc_arch/hosting-and-cicd.md` の更新が別途必要になる。

## Sources

- [Self-Hosting | Supabase Docs](https://supabase.com/docs/guides/self-hosting)
- [GitHub - supabase-community/supabase-kubernetes](https://github.com/supabase-community/supabase-kubernetes)
- [supabase-kubernetes/README.md — GitHub](https://github.com/supabase-community/supabase-kubernetes/blob/main/README.md)
- [supabase-kubernetes/charts/supabase/values.yaml — GitHub](https://github.com/supabase-community/supabase-kubernetes/blob/main/charts/supabase/values.yaml)
- [supabase-kubernetes/config/samples/project.yaml — GitHub](https://github.com/supabase-community/supabase-kubernetes/blob/main/config/samples/project.yaml)
- [supabase-kubernetes/config/samples/singledatabase.yaml — GitHub](https://github.com/supabase-community/supabase-kubernetes/blob/main/config/samples/singledatabase.yaml)
- [supabase-kubernetes releases — GitHub API](https://api.github.com/repos/supabase-community/supabase-kubernetes/releases)
- [supabase-kubernetes commits — GitHub API](https://api.github.com/repos/supabase-community/supabase-kubernetes/commits)
- [Self-hosting: What's working (and what's not)? · supabase · Discussion #39820](https://github.com/orgs/supabase/discussions/39820)
- [Support for Kubernetes Postgres Operators (CNPG) · supabase · Discussion #31147](https://github.com/orgs/supabase/discussions/31147)
- [Supabase on Kubernetes: A Scalable Open-Source Firebase Alternative — UpCloud Docs（二次情報）](https://upcloud.com/docs/guides/supabase/)
- [az containerapp compose — Microsoft Learn（一次情報、GAステータス確認）](https://learn.microsoft.com/en-us/cli/azure/containerapp/compose?view=azure-cli-latest)
- [Automated software delivery using Docker Compose and Amazon ECS — AWS公式ブログ（廃止前の記録）](https://aws.amazon.com/blogs/containers/automated-software-delivery-using-docker-compose-and-amazon-ecs/)
- [Self-Hosting Supabase with Dokploy: Complete Setup on HA Infrastructure — MassiveGRID Blog（二次情報）](https://massivegrid.com/blog/self-hosting-supabase-with-dokploy/)
- [Deploying Self-Hosted Supabase on Coolify: A Complete Guide — Supascale Blog（二次情報）](https://www.supascale.app/blog/deploying-selfhosted-supabase-on-coolify-a-complete-guide)

## 関連ドキュメント

- `docs_bevy_sample/20260816_supabase-api-gateway-kong-to-envoy.md`
- `docs_bevy_sample/20260816_gotrue-overview-and-supabase-cloud-pricing.md`
- `docs_bevy_sample/20260816_cloudnativepg-cnpg-overview.md`（§7 のCNPG統合要望を深掘りしたノート）
- `doc_arch/backend.md`（§1: Supabase＋DynamoDBハイブリッド採用の決定事項）
- `doc_arch/overview.md`（本番ホスティング方式が未決事項として残っている箇所）
