# Supabase self-hosted構成のAPIゲートウェイ（Kong / Envoy）まとめ

作成日: 2026-08-16
出典: GitHub公式リポジトリ・Supabase公式Changelog・Lyft公式Engineeringブログ（いずれも
一次情報）。下部の Sources を参照。

## 前提

- 本ノートは bevy_sample プロジェクト（Bevy 製ブロック崩しを WASM 化し React フロントから
  起動するサンプル。Web 公開・UGC 基盤を構築中）における調査記録。
- 調査のきっかけは、Supabase の構成要素を調べる過程で出てきた「そもそも Kong とは何だったのか」
  「Envoy とはどういうサービスか」「Envoy は今 Go で書かれているのではないか」という一連の
  疑問。
- 関連ノート: `docs_bevy_sample/20260816_gotrue-overview-and-supabase-cloud-pricing.md`
  （同日に調べた Supabase Auth／GoTrue と Cloud 課金の調査）。

## Kongとは何か

Kong Gateway は、クラウドネイティブで拡張性の高い API ゲートウェイ製品。複数のバックエンド
サービスの前段に立ち、リクエストのパスなどを見て適切なサービスへ振り分ける「受付窓口」の役割を
果たす。認証チェック・レート制限・ロギングといった全サービス共通の処理もここに集約できる。

技術的には、Nginx の上に OpenResty（Nginx を拡張して Lua スクリプトを実行できるようにした
もの）を載せ、その上で Kong 自身のロジックを Lua で実装したアプリケーションである。Nginx が
高速にソケットを捌き、その先の処理判断を Lua のプラグイン機構に委ねる構成で、プラグインを
書くことで機能を拡張できる。ライセンスは Apache License 2.0。

## Supabase self-hosted構成でのKongの役割

Supabase の標準的な self-hosted 構成（`docker-compose.yml` で Postgres・Auth（GoTrue）・
PostgREST・Storage・Realtime など複数コンテナを立てる構成）では、Kong がそれら全部の前段に
立つ唯一の入り口だった。クライアントは各サービスのポートに直接つなぐのではなく、まず Kong に
アクセスし、Kong がパスで振り分ける（例: `/auth/v1/*` は GoTrue、`/rest/v1/*` は PostgREST）。
あわせて `anon` / `service_role` キーの検証も Kong が担っていた。

## KongからEnvoyへの移行（2026年8月9日の週〜）

Supabase 公式 Changelog によると、2026 年 8 月 9 日の週から、self-hosted 構成のデフォルト
ゲートウェイが Kong から Envoy に切り替わった。

### 移行の理由

- Kong のオープンソース版が事実上開発停滞状態にあり、採用バージョン（`kong:3.9.1`）も約 1 年
  前のもので、以降のリリースはセキュリティ修正のみ。Kong は 3.10 で無償モードの機能を廃止する
  など、長期的なセキュリティリスクが増大していた。
- Envoy への移行により、ルーティングやアクセス制御が `volumes/api/envoy/` 配下のバージョン
  管理された YAML に統一される。
- 新しい API キーフォーマット（`sb_publishable_*` / `sb_secret_*`）への一級市民としての対応、
  パス正規化やヘッダー検査などより堅牢なデフォルト設定も理由に挙げられている。

### スケジュール

- 2026 年 7 月 17 日: 移行のお知らせを公開
- 2026 年 8 月中旬頃: ドキュメント更新と実装リリース
- 2026 年 8 月 9 日の週: デフォルトゲートウェイが Kong から Envoy に変更

### 破壊的変更の内容

| 対象 | 影響の詳細 |
| --- | --- |
| HTTPS リスナー | Kong の 8443 ポートが廃止。Caddy/Nginx を使うか Kong を再度有効化する必要がある |
| カスタム設定 | 独自の `kong.yml` はサイレント無効化される。Envoy 設定への移行、または Kong の再有効化が必要 |
| スクリプト | `docker compose logs kong` 等サービス名を直接参照するコマンドが動作しなくなる。新しい名前は `api-gw` / `supabase-envoy` |

ただし後方互換性として `kong` はネットワークエイリアスとして機能するため、ホスト名での参照は
維持される。

### Kongの今後

Kong は完全に削除されるのではなく、`sh run.sh config add kong` で有効化する
`docker-compose.kong.yml` によるオプトイン設定として継続提供される。ただし Supabase は
これを「移行補助としての一時的な選択肢」と位置づけており、長期的なデフォルトとしては
推奨していない。

なお、この移行は Supabase Cloud（マネージドサービス）の利用者や、ローカル開発で Supabase
CLI を使っている場合には影響しない。self-hosted の Docker Compose 構成を直接触っている
場合のみの話である。

## Envoyとは何か

配車サービスの Lyft が、自社のマイクロサービス基盤における可観測性・信頼性の課題を解決する
ために自社開発したプロキシ。現在は CNCF（Cloud Native Computing Foundation。Kubernetes等も
所属する中立的な財団）にホストされているオープンソースプロジェクトで、特定企業の製品ではなく
コミュニティ主導で開発が続いている。

- 高性能な C++ 実装（後述）。メモリフットプリントが小さく、どんな言語で書かれたアプリケーション
  の隣にでも配置できる。
- HTTP/2・HTTP/3・gRPC に対応し、通信内容を深く理解した上での可視化（詳細な統計情報）ができる
  L7（アプリケーション層）プロキシ。JWT 認証・RBAC・外部認可などのセキュリティ機能もフィルタ
  として組み込める。
- 単体のゲートウェイとしてだけでなく、サービスメッシュ（多数のマイクロサービス間の通信を管理
  する基盤）のデータプレーンとしても広く使われる（例: Istio の内部エンジンは Envoy）。

## Envoy本体とコントロールプレーンの実装言語（GoとC++の混同ポイント）

Envoy のエコシステムには、実際にトラフィックを処理する「データプレーン」と、Envoy に設定を
動的配信する「コントロールプレーン」という 2 つの層があり、実装言語が異なる。

| コンポーネント | 役割 | 実装言語 |
| --- | --- | --- |
| `envoyproxy/envoy`（Envoy 本体） | 実際にリクエストを受けて捌くデータプレーン | C++ |
| `envoyproxy/go-control-plane` | Envoy に設定を動的配信する xDS プロトコルの実装 | Go |
| `envoyproxy/gateway`（Envoy Gateway） | Kubernetes 上で Envoy を管理するコントロールプレーン。`go-control-plane` ベース | Go |

Envoy 本体が C++ であることは、プロジェクト発表元である Lyft Engineering の公式ブログで
明記されている："Envoy is a high performance C++ distributed proxy originally built at
Lyft... Envoy is written in C++11, for performance reasons."

一方、Envoy をデータプレーンとして使う周辺のコントロールプレーン（`go-control-plane` や
Envoy Gateway、Istio の istiod 等）は軒並み Go で書かれている。「Envoy は Go」という
イメージは、この周辺のコントロールプレーン層との混同によるものと考えられる。

## 本プロジェクトでの位置づけ

本プロジェクトの `docker-compose.yml` は、公式スタックから `db`・`rest`・`auth` だけを残し、
Kong・Storage・Realtime・Studio 等は意図的に定義していない（冒頭のコメントに明記）。
「認証（OAuth ログイン）以外は REST API 経由でのみ DB に触る」という方針のため、Kong や
Envoy のような統一ゲートウェイ自体が不要と判断されている。

代わりに `auth-gateway` という最小の nginx プロキシを自前で用意し、`supabase-js` が
`/auth/v1` プレフィックス固定でリクエストを投げてくる仕様に合わせて、そのパスだけを GoTrue へ
転送している。つまり本プロジェクトは、Kong を Envoy に置き換えたのではなく、そもそも Kong の
役割自体を最小限の自前プロキシで代替している、という位置づけである。

## 読み方の注意

- Kong・Envoy 本体・移行スケジュールに関する内容は、いずれも一次情報（GitHub 公式リポジトリ
  `Kong/kong` / `envoyproxy/envoy` / `envoyproxy/gateway` / `envoyproxy/go-control-plane`、
  Supabase 公式 Changelog、Lyft Engineering 公式ブログ）から確認したものであり、解説ブログ等の
  二次情報は参照していない。
- 本プロジェクトが実際に Envoy の xDS コントロールプレーン（動的設定配信の仕組み）を使って
  いるかどうかは未確認。Supabase 公式 Changelog の「`volumes/api/envoy/` 配下のバージョン
  管理された YAML」という記述から、静的な設定ファイルで Envoy 本体（C++）を動かす構成である
  可能性が高いと推測しているに過ぎない（この点は推測であり断定ではない）。
- 移行のスケジュール・破壊的変更の内容は取得日（2026-08-16）時点の Changelog の記載であり、
  実際の展開状況は変わり得る。

## Sources

- [GitHub - Kong/kong](https://github.com/Kong/kong)
- [Self-hosted Supabase: Envoy becomes the default API gateway (breaking change) — Supabase Changelog](https://supabase.com/changelog/48048-self-hosted-supabase-envoy-becomes-the-default-api-gateway-b)
- [GitHub - envoyproxy/envoy](https://github.com/envoyproxy/envoy)
- [Envoy Proxy 公式サイト](https://www.envoyproxy.io/)
- [Announcing Envoy: C++ L7 proxy and communication bus — Lyft Engineering](https://eng.lyft.com/announcing-envoy-c-l7-proxy-and-communication-bus-92520b6c8191)
- [GitHub - envoyproxy/go-control-plane](https://github.com/envoyproxy/go-control-plane)
- [GitHub - envoyproxy/gateway](https://github.com/envoyproxy/gateway)

## 関連ドキュメント

- `docs_bevy_sample/20260816_gotrue-overview-and-supabase-cloud-pricing.md`
- `docs_bevy_sample/20260816_kubernetes-supabase-self-hosting.md`（この Kong→Envoy 移行が
  Kubernetes 版セルフホストの実装にもどう反映されているかを調査したノート）
- `docker-compose.yml`（本プロジェクトの `auth-gateway` 定義、Kong を使わない理由のコメント）
