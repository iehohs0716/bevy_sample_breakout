# ローカルGoTrue + Google ログイン サンドボックス検証のトラブルシューティング記録

作成日: 2026-08-16

## 1. 背景

- 元の `docker-compose.yml` は「Supabase を素の Postgres + REST（PostgREST）としてのみ使う」
  最小構成（`db` / `rest` の2サービスのみ）だった。
- `doc_arch/backend.md` §1 で認証・ユーザー情報に Supabase Auth を使う方針が確定していたため、
  ローカル検証用に `auth`（GoTrue）サービスを追加することになった。
- 本家 Supabase 構成が使う Kong は導入せず、軽量な nginx コンテナ（`auth-gateway`）で
  `/auth/v1/*` のパス剥がしと CORS 処理を代用する方針を採った（Kong のフル機能は今回の
  検証範囲では不要という判断）。
- 検証用フロントは `frontend/src/pages/oauth-sandbox` に新規作成した FSD の `pages` スライス。

## 2. 起きたトラブルと対処（時系列）

### (1) `/auth/v1/*` 404
`@supabase/supabase-js` の `createClient()` は `${url}/auth/v1/...` という固定パスを叩く実装
（Auth API のベース URL をオプションで上書きできない）。GoTrue をそのまま公開しただけでは
GoTrue 自身がこのプレフィックスを知らないため 404 になる。

→ `supabase-local/volumes/auth-gateway/nginx.conf` で `location /auth/v1/` の
`rewrite ^/auth/v1/(.*)$ /$1 break;` によりプレフィックスを剥がし、`auth:9999` へ
`proxy_pass` して対処。

### (2) OPTIONSプリフライトにCORSヘッダーが付かない
GoTrue は実際の POST/GET レスポンスには `Access-Control-Allow-Origin: *` 等を自ら返すが、
OPTIONS プリフライトには何も返さないことを curl で実機確認した。ブラウザからの
`exchangeCodeForSession` 等の fetch が CORS エラー（Failed to fetch）になった。

→ nginx 側で OPTIONS プリフライトへの CORS ヘッダー付与処理（`add_header
Access-Control-Allow-*` と `return 204`）を追加。

### (3) CORSヘッダーの重複
GoTrue 自身が一部レスポンスで返す CORS ヘッダーと、nginx が `add_header` で追加した CORS
ヘッダーが重複し、ブラウザに CORS エラー扱いされた。

→ `proxy_hide_header` で `Access-Control-Allow-Origin` / `-Credentials` / `-Methods` /
`-Headers` / `-Expose-Headers` を一旦隠してから、nginx 側で付け直す方式に修正。

### (4) 固定Allow-Headersリストの追従漏れ
`Access-Control-Allow-Headers` を `Authorization, Content-Type, apikey, x-client-info` の
固定リストにしていたところ、`@supabase/supabase-js`（v2.112.3）が実際に送る
`x-supabase-api-version` ヘッダーが含まれておらず CORS エラーになった。

→ 固定リストをやめ、`$http_access_control_request_headers`（ブラウザが実際に要求した
ヘッダーをそのまま反映）に変更。

### (5) StrictModeによるPKCE二重消費（インフラとは別原因）
React の開発モード（`StrictMode`）は `useEffect` を意図的に2回実行する仕様があり、
`exchangeCodeForSession` が2回呼ばれた結果、PKCE の `code_verifier` が1回目の完了時に
storage から削除され、2回目が `pkce_code_verifier_not_found` エラーになっていた。

→ `frontend/src/pages/oauth-sandbox/ui/OAuthSandboxPage.tsx` で `useRef`
（`hasExchangedCodeRef`）を使い、コード交換処理を1回だけ実行するよう修正。

## 3. 事後調査で判明した根本原因の切り分け（一次情報ベース）

Supabase 公式リポジトリ `supabase/supabase` の `docker/volumes/api/kong.yml`、Kong 本体
（`Kong/kong`）の `kong/plugins/cors/handler.lua` と `schema.lua`、および GoTrue 改め
`supabase/auth` リポジトリの `internal/api/api.go`（`github.com/rs/cors` 使用）を実際に
確認した。

- **問題(1)（パス剥がし）**: Kong 構成でも `strip_path: true` が明示的に必要な設定であり、
  Kong / nginx どちらを使っても同様に発生する。ゲートウェイの選択とは無関係。
- **問題(3)・(4)**: Kong の `cors` プラグインが標準で持つ設計 —— `headers` 未指定時は
  ブラウザの `Access-Control-Request-Headers` をそのまま動的に反映する、`set_header` で
  CORS ヘッダーを上書き一元管理して重複を防ぐ —— を把握しないまま、nginx で固定リスト・
  `add_header`（追加のみ）という素朴な実装を最初に選んでしまったことが直接原因。Kong を
  使っていれば標準動作としてこの2つは最初から発生しなかった。
- **問題(2)**: GoTrue（`rs/cors`）・Kong（`cors` プラグイン）ともに「OPTIONS ＋ Origin ＋
  Access-Control-Request-Method の3条件が揃わないと CORS ヘッダーを返さない」という
  同一ロジックを持つ。しかし実際に GoTrue へこの3条件を満たした OPTIONS リクエストを
  送っても（実機の docker 環境で）CORS ヘッダーが返ってこなかった。コード上の理論と実機の
  挙動に矛盾があり、**原因は完全には特定できていない**（未解明点）。
- **結論**: 「Kong を使わず nginx で代用したこと自体」は根本原因ではなく妥当な
  トレードオフだったが、「CORS 処理を nginx で自作する際、Kong の cors プラグインが標準で
  持つデフォルト設計を踏まえずに実装した考慮漏れ」が問題(3)・(4)の直接原因。加えて、
  React 側の StrictMode 二重実行というインフラと独立したバグ（問題(5)）も複合的に
  絡んでいた。

## 4. 今後の教訓

- Kong のような既存ゲートウェイの機能を自作コンポーネント（nginx 等）で代替する際は、
  「代替対象が標準で何をデフォルトでやっているか」を先に一次情報（実装コード）で確認して
  から実装する方が、後追いの修正回数を減らせる。
- CORS の `Allow-Headers` は固定リストで持たず、`Access-Control-Request-Headers` を
  そのまま反映する方式にしておくと、クライアントライブラリのバージョンアップによる
  ヘッダー追従漏れを防げる。

## 5. StrictModeのuseEffect二重実行「修正」の妥当性の再検証

トラブルシューティング中に行った1つの対処（React の `useEffect` 二重実行への `useRef`
によるガード追加、上記2.(5)）について、それが実際に今回のログイン失敗の直接原因を解決した
ものだったのかを、後から振り返って正直に検証した結果を記録する。目的は「状況証拠だけで
原因を確定させて次に進んでしまう」という進め方の反省を残すこと。

### 何が起きていたか（事実）

- GoTrue のアクセスログで、Google からの callback 後に `OPTIONS /auth/v1/token`
  リクエストが2回連続で記録されていた。
- これは React 19 の開発モード（StrictMode）が `useEffect` を意図的に「mount→cleanup→mount」
  の順で2回実行する仕様と一致しており、`OAuthSandboxPage` の `useEffect` 内で呼んでいた
  `supabaseAuthClient.auth.exchangeCodeForSession(code)` が実際に2回呼ばれていたこと自体は、
  ログの記録から見て事実だった。

### 行った対処

- `frontend/src/pages/oauth-sandbox/ui/OAuthSandboxPage.tsx` に `useRef`
  （`hasExchangedCodeRef`）を追加し、`exchangeCodeForSession` の呼び出しを1回目の
  `useEffect` 実行時のみに制限した。

### 「原因だ」と判断した根拠（そしてそれがなぜ不十分だったか）

- 判断の決め手にしたのは、`signInWithOAuth` を呼ばずに（つまり PKCE の code_verifier を
  localStorage に保存する手順を経ずに）、ダミーの文字列コードで
  `supabaseAuthClient.auth.exchangeCodeForSession('dummy-...')` を手動実行した結果、
  `pkce_code_verifier_not_found` というエラーが返ってきたこと。
- しかし、このエラーは「そもそも verifier を保存する手順（`signInWithOAuth`）を踏んでいない
  テストだったから」起きるのが当然のエラーであり、「StrictMode の二重 `useEffect` 実行が
  実際のログインフローで同じエラーを引き起こしている」ことを示す直接証拠にはなっていなかった。
  無関係な現象を、確証があるかのように扱ってしまっていた。
- `@supabase/supabase-js` のソース（`node_modules/@supabase/supabase-js/dist/umd/supabase.js`
  内の `_exchangeCodeForSession` 実装）を読み、「code_verifier はリクエスト完了後（成功時・
  失敗時どちらも）に storage から削除される」実装であることは事実として確認したが、これは
  「2回呼ばれたら2回目が失敗する」という一般論の裏付けにはなっても、「今回の症状がまさに
  これで起きていた」という証明にはならなかった。

### 結局、この修正は必要だったのか（結論）

- **今回の一連の失敗の直接原因ではなかった可能性が高い**、というのが事後の正直な評価。
- この対処を行った時点（トラブルシューティングの中盤）では、後に発覚する真因——固定の
  `Access-Control-Allow-Headers` リストに `x-supabase-api-version` ヘッダーが欠けていた
  ための CORS エラー（上記2.(4)）——がまだ残っていた。ブラウザは CORS プリフライトの時点で
  実際の POST リクエストをブロックしていたはずであり、「1回目のリクエストが成功して
  verifier を消費し、2回目が失敗する」というシナリオが発生する前提条件（1回目が
  成功していること）自体が満たされていなかった。
- `useRef` による1回制限自体は、副作用を伴う非同期処理を StrictMode の二重実行から守る
  一般的に正しいプラクティスであり、コードとして残すこと自体に害はない。ただし「これが
  当時の症状を解決した」という説明は誤りで、実際に症状を解決したのは後から見つかった
  CORS ヘッダーの修正（上記2.(4)）だった。

**追記（実証結果）**: 上記2.(4)の CORS 修正後、`useRef` によるガードを
`frontend/src/pages/oauth-sandbox/ui/OAuthSandboxPage.tsx` から削除し、素の（StrictMode の
二重実行を許した）状態で実際にログインを再テストした。GoTrue のログを確認したところ、
`POST /auth/v1/token?grant_type=pkce` が同一の認可コードに対して**2回とも `200` で成功**
しており、`pkce_code_verifier_not_found` のようなエラーは一切発生しなかった。つまり
GoTrue 側は同一コードへの重複リクエストを問題なく処理する実装になっており、`useRef` に
よる対策は実際に不要だったことが実機で確認された。上の「一般的に正しいプラクティスだが
コード上に残すこと自体に害はない」という評価は、本件では「今回はそもそも不要な複雑さ
だった」に訂正する（CLAUDE.mdの「過剰な実装をしない」方針にも合わせ、コードからは削除済み）。

### 教訓（How to applyとして）

- ログに現れた「状況証拠」（今回で言えば OPTIONS リクエストの重複）と、それを踏まえて
  行った再現テストの結果を混同しないこと。再現テストは「疑っている原因を再現する条件を
  正確に揃えて」行わないと、無関係な現象を証拠として採用してしまう。
- 1つの仮説を検証する前に、他に残っている既知の問題（このケースでは未解決だった CORS
  エラー）が、検証結果を汚染していないかを確認する。複数の問題が同時に存在する状況では、
  ある対処の効果測定は、他の問題が全て解消された後でないと正確に行えない。

## 関連ドキュメント

- `docker-compose.yml`（`auth-gateway` / `auth` サービス定義）
- `supabase-local/volumes/auth-gateway/nginx.conf`（本記録で最終的に到達した nginx 設定）
- `frontend/src/pages/oauth-sandbox/`（検証用フロント。`ui/OAuthSandboxPage.tsx` に
  StrictMode対策の `useRef` 実装がある）
- `docs_bevy_sample/20260816_google-oauth-client-setup-for-local-gotrue.md`（Google Cloud
  Console 側の手動セットアップ手順）
- `docs_bevy_sample/20260816_gotrue-overview-and-supabase-cloud-pricing.md`（GoTrueの由来と
  Supabase Cloud課金の調査記録）
