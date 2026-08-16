# GoTrueとSupabase Cloud運用時の課金 まとめ

作成日: 2026-08-16
出典: GitHub公式リポジトリ・Supabase公式ドキュメント（いずれも一次情報）。下部の Sources を
参照。

## 前提

- 本ノートは bevy_sample プロジェクト（Bevy 製ブロック崩しを WASM 化し React フロントから
  起動するサンプル。Web 公開・UGC 基盤を構築中）における調査記録。
- 調査のきっかけは「Supabase の GoTrue とは何か」「Cloud 運用した場合に課金されるか」という
  疑問。
- reading-notes リポジトリがこの作業マシン上に見つからなかったため、本プロジェクトの設計議論
  ログの置き場である `docs_bevy_sample/` に、同フォルダの命名規則
  （`YYYYMMDD_kebab-case.md`）で保存している。

## GoTrueの由来とSupabase Authへの変遷

- GoTrue はもともと Netlify が作った独立 OSS プロジェクト（`netlify/gotrue`）。Go 言語で
  書かれた、JWT ベースのユーザー管理・認証 API であり、Jamstack プロジェクト向けの自立型認証
  サービスとして設計されている。OAuth2 と JWT に基づき、サインアップ・認証・カスタムユーザー
  データの管理を行う。メールベースのパスワードリカバリー、複数の外部認証プロバイダー
  （GitHub・GitLab・Google・Bitbucket 等）、Webhook サポートを持つ。取得時点で 4.5k star・
  328 fork。
- Supabase はこの GoTrue をフォークして自社の認証サービスとして採用したが、その後「機能と
  性能の面で大きく分岐した」（GitHub `supabase/auth` の説明文より）。
- 分岐が進んだ結果、Supabase 側のリポジトリ名は `supabase/gotrue` から `supabase/auth` へ
  リネームされた。ただし Docker イメージ（`supabase/gotrue`）やクライアントライブラリ
  （`@supabase/gotrue-js`）は引き続き公開されており、v2 の間は `@supabase/auth-js` と
  互換性がある（破壊的変更なし）。リネームの理由自体は README 上に明示的な記載がなく、
  「機能範囲（メール・電話・OAuth 連携など）をより正確に反映する名称にした」という理解に
  とどまる（推測）。
- つまり「GoTrue」という語は現在、(1) Netlify のオリジナルプロジェクト名、(2) Supabase Auth
  のリブランディング前の旧称、(3) 今も公開され続ける Docker イメージ／パッケージ名、という
  3 つの意味で使われている。本プロジェクトの `docker-compose.yml` の `auth` サービスは (2)(3)
  の系譜にある Supabase Auth（GoTrue）の実体。

## Supabase Authのアーキテクチャ

Supabase 公式ドキュメント（Auth architecture）によれば、Supabase Auth は 4 層構成。

1. **クライアント層**: Supabase の各言語向け Client SDK、または任意の HTTP クライアント。
2. **Envoy API ゲートウェイ**: Supabase の全製品で共有されるリバースプロキシ。
3. **Auth Service**: Supabase が開発・保守する認証 API サーバー本体（＝GoTrue のフォーク）。
4. **Postgres データベース**: 全製品で共有する統一 DB インスタンス。

Auth Service はアプリと Postgres 内の認証情報の仲介役であり、Postgres の `auth` スキーマに
ユーザーデータを保存する。ここに保存されたユーザー情報は、トリガーや外部キー参照で自分の
テーブルと接続でき、Row Level Security（RLS）による認可とも連携する。

## 本プロジェクトでの位置づけ

- `doc_arch/backend.md` §1 で「認証・ユーザー情報は Supabase」という確定方針になっており、その
  実装が GoTrue（Supabase Auth）。
- 同§3 で、フロントから `supabase-js` で GoTrue を直叩きするのではなく、自前 API 層
  （Cloudflare Workers）を挟んで Supabase 固有仕様（JWT 発行元など）を隠蔽する方針
  （BaaS ベンダーロックイン回避のため）。GoTrue は「現在使っている認証エンジン」ではあるが、
  将来差し替え可能なように設計上は距離を置く位置づけになっている。
- 関連ドキュメント: `doc_arch/backend.md`（§1・§3・§4 の Auth.js 検討）、
  `docs_bevy_sample/20260731_auth0-supabase-third-party-auth.md`（Auth0 を IdP にする場合の
  代替案の検討記録、未決）、`docs_bevy_sample/20260816_google-oauth-client-setup-for-local-gotrue.md`
  （ローカル GoTrue での Google OAuth 設定手順）。

## Supabase Cloud運用時の課金

**結論**: Cloud 環境（Supabase 社がホストするプロジェクト）で GoTrue（Supabase Auth）を使う
場合は課金対象になる。ローカルの Docker Compose 上の GoTrue（自前ホスティング）自体には課金は
発生しない。課金が発生するのは Supabase 社の Cloud 環境に乗せた場合のみ。

課金は MAU（Monthly Active Users）に基づく従量制。MAU の定義は、当月の請求サイクル中に
ログインまたはトークンリフレッシュを行った重複なしユーザー数（Google 等ソーシャルログインも
対象）。同一ユーザーは 1 請求サイクル中 1 回のみカウントされ、サイクルの開始時にカウントは
リセットされる。

### プラン別月額とMAU無料枠

| プラン | 月額 | MAU無料枠 |
| --- | --- | --- |
| Free | $0/月 | 50,000人まで無料 |
| Pro | $25/月〜 | 100,000人まで含む |
| Team | $599/月〜 | 100,000人まで含む |
| Enterprise | カスタム | カスタム |

### 超過時の単価

1MAU あたり $0.00325（例: 100,000 人枠を 60,000 人超過した月は 60,000 × $0.00325 ≒ $195 の
追加）。

### SSO（SAML 2.0）は別枠

50 MAU まで無料、以降 1MAU あたり $0.015。通常の Google ログイン等ソーシャルログインは通常
MAU 枠であり、SSO 枠ではない。

### Third-Party MAU

Clerk / Firebase Auth / Auth0 等の外部認証プロバイダ経由の場合も、通常 MAU と同様に無料枠
100,000 人、超過 $0.00325/MAU。

### Advanced MFA（電話番号認証）

Pro/Team プランでは、電話番号による多要素認証を有効にすると、最初のプロジェクトが
$75/月、追加プロジェクトごとに $10/月の固定費用が別途かかる（MAU 従量課金とは別枠）。

### 補足・注意点

開発中の動作確認ログインも MAU にカウントされる点は、本番移行時の注意点として留意する。

## 読み方の注意

- GoTrue の由来・リネーム経緯は GitHub 公式リポジトリ（`supabase/auth`・`netlify/gotrue`・
  リネームを議論する GitHub Discussion）という一次情報から確認した。アーキテクチャと課金体系は
  いずれも Supabase 公式ドキュメントという一次情報から確認した。解説ブログ等の二次情報は
  参照していない。
- リネームの理由そのものは公式ドキュメント上に明記がなく、本ノートの記述は推測であることを
  明示している（上記「GoTrue の由来と Supabase Auth への変遷」参照）。
- 料金は取得日（2026-08-16）時点のもの。プランや単価は変更され得るため、実際に Cloud 移行を
  判断する際は最新の pricing ページを再確認すること。

## Sources

- [GitHub - supabase/auth](https://github.com/supabase/auth)
- [GitHub - netlify/gotrue](https://github.com/netlify/gotrue)
- [whats the difference between the auth and gotrue docker images? — Supabase GitHub Discussion #21364](https://github.com/orgs/supabase/discussions/21364)
- [Auth architecture — Supabase Docs](https://supabase.com/docs/guides/auth/architecture)
- [Manage Monthly Active Users usage — Supabase Docs](https://supabase.com/docs/guides/platform/manage-your-usage/monthly-active-users)
- [Supabase Pricing — Supabase](https://supabase.com/pricing)

## 関連ドキュメント（本プロジェクト内）

- `doc_arch/backend.md` §1・§3（認証に Supabase Auth を使う確定方針、自前 API 層との
  ポータビリティ方針）
- `docs_bevy_sample/20260816_google-oauth-client-setup-for-local-gotrue.md`
- `docs_bevy_sample/20260816_google-oauth-client-setup-for-supabase-cloud.md`
- `docs_bevy_sample/20260731_auth0-supabase-third-party-auth.md`
- `docs_bevy_sample/20260816_supabase-api-gateway-kong-to-envoy.md`（同日に調べた
  Supabase self-hosted 構成の API ゲートウェイ（Kong / Envoy）の調査）
