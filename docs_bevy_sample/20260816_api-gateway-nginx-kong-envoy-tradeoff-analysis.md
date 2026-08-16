# APIゲートウェイ選定（nginx / Kong / Envoy）のトレードオフ分析

作成日: 2026-08-16
出典: Supabase公式チェンジログ・公式ドキュメント、Kong/Envoy公式リポジトリのソースコード（一次情報）。下部の Sources を参照。

## 前提

- 対象は、ローカルのSupabase Auth(GoTrue)検証環境で `/auth/v1/*` のパス変換とCORS処理を担う
  APIゲートウェイ（現状は自作nginx、`supabase-local/volumes/auth-gateway/nginx.conf`）。
- 現状のnginx実装に至るまでに、CORS関連で3つのバグを踏み、追加修正を重ねた経緯がある
  （詳細: `docs_bevy_sample/20260816_local-gotrue-google-login-troubleshooting.md`）。
  - ①GoTrueがOPTIONSプリフライトにCORSヘッダーを返さない
  - ②GoTrueとnginx双方がCORSヘッダーを返し重複する
  - ③固定の`Access-Control-Allow-Headers`リストに`x-supabase-api-version`が漏れていた
- この経験を踏まえ、「Kongに乗り換えるべきでは」という提案が出た。調査の過程で、
  **Supabase公式が2026年8月9日週からセルフホストのデフォルトAPIゲートウェイをKongから
  Envoyに変更し、Kongを新規採用非推奨とした**事実が判明したため、nginx・Kong・Envoyの
  3者で比較する。

## 1. CORS設定の堅牢性・保守性（①〜③の再発可能性）

| バグ | nginx（現状） | Kong | Envoy |
|---|---|---|---|
| ①OPTIONSにCORSヘッダーが付かない | GoTrueの未解明挙動に対し、nginxが`add_header`+`return 204`で代行する後付け実装 | corsプラグインが「OPTIONS＋Origin＋Access-Control-Request-Methodの3条件成立時」に**Kong自身が応答を生成**し、GoTrueへ転送しない設計。GoTrue固有の未解明挙動から切り離される | EnvoyのCORSフィルタも同様にroute単位でプリフライトへ**Envoy自身が応答**する設計 |
| ②CORSヘッダーの重複 | `proxy_hide_header`でGoTrue側のヘッダーを隠してから`add_header`で付け直す、後付けの手当てが必要だった | `set_header`（上書き）による一元管理が標準動作のため、そもそも重複が起きない | 公式テンプレートでCORSヘッダーが一元管理されている前提であれば重複しない（実装の詳細確認は必要） |
| ③固定Allow-Headersリストの追従漏れ | 当初`Authorization, Content-Type, apikey, x-client-info`の固定リストで実装し`x-supabase-api-version`が漏れた。最終的に`$http_access_control_request_headers`（ブラウザが実際に要求したヘッダーをそのまま反映）へ変更して解消 | `headers`未指定時はこの動的反映が標準動作。固定リストを書かない限り再発しない（誰かが明示的に`headers:`を指定すると再発しうる） | `allow_headers: "*"`のワイルドカード全許可。「リストに漏れる」という事象自体が構造上発生しない |

**総評**: 現状のnginx実装は、3回の追加修正を経て**Kongの標準設計思想に事後的に追いついた**状態。Kong・Envoyはこれらを標準機能として最初から回避する設計になっている。

## 2. 今回の規模（GoTrue1サービスのみ）に対する設定量・複雑さ

| 観点 | nginx（現状） | Kong | Envoy |
|---|---|---|---|
| 設定ファイル数 | 1ファイル（53行） | 1ファイル（`kong.yml`）＋compose変更 | 4ファイル（`envoy.yaml`/`cds.yaml`/`lds.template.yaml`/`docker-entrypoint.sh`）＋compose変更 |
| 今回必要な作業量 | 変更なし（既に稼働中） | 新規`kong.yml`作成（authルートのみなら小規模） | 公式テンプレートが前提とする**7クラスタ→1クラスタ、フルスタックroute定義→authルートのみ**へ大幅に削る作業が必要 |
| 概念の複雑さ | nginxの基本構文のみ | Kongの宣言的設定（DB-lessモード）＋プラグインDSL | xDS（Cluster/Listener Discovery Service）、RBAC、Luaフィルタなど概念数が多い |

**総評**: 最小規模に対しては現状のnginxが最も軽量。Envoyは「最小構成にするために公式テンプレートを大きく削る」という逆転作業が発生し、今回の規模には最も不釣り合い。

### 補足: 「最小構成に絞る」という前提を外した場合

上記の評価は「今回使わないサービスの定義は削るべき」という前提に立っている。しかし、Supabase公式ドキュメントには「不要なサービス（Realtime/Storage/imgproxy/Edge Runtime）は`docker-compose.yml`から削除してリソースを削減**できる**」という記述があり、裏を返せば削除は必須ではない。

Envoyのクラスタはそれぞれ独立して状態管理されるため、`cds.yaml`に定義されたクラスタのうち実体が存在しないもの（今回で言えばrealtime/storage/functions/meta/studio）は、ヘルスチェックが失敗し続けるだけで、authクラスタへのルーティングなど他の機能には影響しない。実害は「存在しないアップストリームへの定期的なヘルスチェックログが出続ける」程度に留まる（Envoy自体の一般的なクラスタ管理アーキテクチャからの推論であり、実機での確認はしていない）。

この前提に立つなら、Envoyの導入コストの評価は大きく変わる。

| 観点 | 最小構成に削る場合 | 公式テンプレートをそのまま使う場合 |
|---|---|---|
| 作業内容 | 7クラスタ→1クラスタ、route定義の大幅な取捨選択が必要 | 公式の`envoy.yaml`/`cds.yaml`/`lds.template.yaml`/`docker-entrypoint.sh`をほぼそのままコピーし、環境変数（`ANON_KEY`等）を設定するだけ |
| 実装の見通しやすさ | 「何を削ってよいか」の判断が必要で、削りすぎ・削り足りなさのリスクがある | 公式が動作確認済みの完成品をそのまま使うため、設定ミスのリスクが低い |
| デメリット | なし（構成は正確にスコープと一致する） | 存在しないサービスへのヘルスチェック失敗ログがノイズになる。将来的にそれらのサービスを本当に追加する予定がなければ、使わない設定を持ち続けることになる |

**この前提に立つ場合、Envoyは「今回の規模には不釣り合い」ではなく、むしろKongより準備の手間が少ない（`kong.yml`は自分で最小構成を書く必要があるが、Envoyは公式一式をそのまま持ってこられる）という評価に変わる。** どちらの前提を取るかは、「今後本当にSupabaseのフルスタック運用に進む可能性が高いか」に依存する（`doc_arch/backend.md`§1でSupabase Auth＋Postgres＋Storageの利用が既に確定方針であるため、可能性は低くない）。

## 3. 将来フルスタック化（rest/storage等追加）時の拡張性

| 観点 | nginx（現状） | Kong | Envoy |
|---|---|---|---|
| サービス追加時の作業 | locationブロック＋CORS処理一式を都度自作・複製する必要がある | 公式`kong.yml`のroute定義パターンをそのまま追加するだけ | 今回削る5〜6クラスタの定義を、**公式テンプレートから復元するだけ** |
| 横断機能（認証・レートリミット等） | 全て自作が必要 | プラグインエコシステムが豊富 | フィルタチェーンで対応可能だが学習コストは高め |

**総評**: 拡張性はnginxが最も低く、サービスが増えるほど保守負担が線形に増える。Envoyは公式の今後のデフォルト構成そのものであり最も拡張性が高い。

## 4. 公式の今後のサポート状況・エコシステムの将来性

| 観点 | Kong | Envoy |
|---|---|---|
| Supabase公式の位置づけ | 2026年8月9日週以降、「transition aid（移行支援の一時的な手段）であり、long-term default（長期的なデフォルト）ではない」と公式が明言 | 2026年8月9日週から公式セルフホストのデフォルトAPIゲートウェイ |
| OSS版の保守状況 | 3.9.x系で1年以上更新停止。3.10以降は無料モード廃止でセキュリティ・コンプライアンスリスクが増加 | 活発に開発が継続 |
| 新APIキー形式（`sb_publishable_*`/`sb_secret_*`）対応 | 一級対応は期待薄（移行支援用途のため） | 一級対応済み（内部JWTへの変換をネイティブサポート） |
| 移行手順の明記状況 | `sh run.sh config add kong` でオーバーライド可能だが「transition aid」と明記 | デフォルトのため追加手順不要 |

**総評**: Kongは下降トレンド、Envoyは上昇トレンド。今からKongを新規採用するのは、公式が手を引きつつあるものを選ぶことになる。

## 5. 学習コスト・追加コンテナのリソースコスト

| 観点 | nginx（現状） | Kong | Envoy |
|---|---|---|---|
| イメージ | `nginx:1.27-alpine`（軽量、既に稼働中） | `kong`（Lua/OpenRestyベース、nginxよりコンテナサイズ・メモリ消費が大きい） | `envoyproxy/envoy`（プロキシ自体は高効率だが、xDS動的設定分の複雑さがある） |
| 学習コスト | 低（nginx基本構文のみ、追加学習ほぼ不要） | 中（宣言的YAML DSL、プラグイン体系） | 高（xDS、RBAC、Luaフィルタ、複数ファイル構成） |
| 追加コンテナ数 | 0（既存） | +1（Kong） | +1（Envoy） |

## 6. 総合的な推奨

「今回使わないサービスの定義は削るべき」という前提を置くかどうかで結論が変わる。

| 前提 | 推奨 | 理由 |
|---|---|---|
| ゲートウェイ設定を今のスコープ（GoTrueのみ）に厳密に一致させる | **nginx現状維持** | ①〜③のCORSバグは解消済みで、Kongの標準設計思想を踏まえた実装になっている。Kong/Envoyへの移行はコンテナ追加・学習コスト・（Envoyの場合）フルスタックテンプレートを大幅に削る作業という投資に見合うリターンが乏しい |
| 「最小構成への限定」にこだわらず、将来のフルスタック化を見据えて構成してよい | **Envoy採用**（§2補足参照） | `doc_arch/backend.md`§1でSupabase Auth＋Postgres＋Storageの利用が既に確定方針であり、将来的にrest/storage等を同じゲートウェイ配下に置く可能性は低くない。公式テンプレートをほぼそのまま持ち込めるため、Kongで`kong.yml`を自作するより準備の手間が少なく、かつ公式の今後のデフォルト構成に最初から乗れる。Kongは新規採用が非推奨なため、経由する意味が薄い |

いずれの前提でも**Kongを新規採用する理由は無い**（今回の規模にはnginxで足りており、フルスタックを見据えるならEnvoyの方が公式の方向性と一致するため）。後者の前提を取る場合の具体的な実装差分は、実装レポート（`docs_bevy_sample/20260816_api-gateway-migration-implementation-options.md`）のパターンCを参照。

この判断は「セキュリティ的な堅牢性」より「今回のプロジェクト規模・検証目的・保守負担」を重視した結論である。本番運用や複数人開発に発展する場合は、Envoyのワイルドカード全許可CORS設定（`allow_headers: "*"`）を絞り込む等の追加検討が別途必要になる。

## 読み方の注意

- Kong公式構成（`kong.yml`）とKong本体のcorsプラグイン実装（`handler.lua`/`schema.lua`）はGitHub上のソースコードを直接確認した一次情報。
- Envoyの設定ファイル（`lds.template.yaml`/`cds.yaml`）もGitHub上のraw文字列を直接取得して確認した一次情報。
- Kong→Envoyのデフォルト変更については、Supabase公式チェンジログ（一次情報）を直接確認済み。
- 「GoTrueがOPTIONSにCORSヘッダーを返さない」という現象について、GoTrue（`supabase/auth`）とKongのcorsプラグインは実装ロジック上は同一の判定条件（OPTIONS＋Origin＋Access-Control-Request-Methodの3条件）を持つことをソースコードで確認したが、実機でGoTrue単体に対しこの3条件を満たすリクエストを送ってもCORSヘッダーが返らなかった実測結果との整合は取れていない。この点は「Kong/Envoyなら再発しない」という記述の確度をやや下げる要素として明記しておく。

## Sources

- [Self-hosted Supabase: Envoy becomes the default API gateway (breaking change) — Supabase Changelog](https://supabase.com/changelog/48048-self-hosted-supabase-envoy-becomes-the-default-api-gateway-b)
- [Envoy API Gateway — Supabase Docs](https://supabase.com/docs/guides/self-hosting/self-hosted-envoy)
- [docker/volumes/api/kong.yml — supabase/supabase (GitHub)](https://github.com/supabase/supabase/blob/master/docker/volumes/api/kong.yml)
- [docker/volumes/api/envoy/lds.template.yaml — supabase/supabase (GitHub)](https://raw.githubusercontent.com/supabase/supabase/master/docker/volumes/api/envoy/lds.template.yaml)
- [docker/volumes/api/envoy/cds.yaml — supabase/supabase (GitHub)](https://raw.githubusercontent.com/supabase/supabase/master/docker/volumes/api/envoy/cds.yaml)
- [kong/plugins/cors/handler.lua, schema.lua — Kong/kong (GitHub)](https://github.com/Kong/kong)
- [internal/api/api.go (CORS実装, rs/cors使用) — supabase/auth (GitHub)](https://github.com/supabase/auth)
- [DB-less mode — Kong Gateway | Kong Docs](https://developer.konghq.com/gateway/db-less-mode/)
- `docs_bevy_sample/20260816_local-gotrue-google-login-troubleshooting.md`（本リポジトリ内、①〜③バグの詳細経緯）
