# 本番（Supabase Cloud）向け Google OAuth クライアント設定手順

日付: 2026-08-16

本ドキュメントは、本番で **Supabase Cloud**（Supabase社のマネージドサービス）を使って
Google ログインを有効化する場合の手順をまとめたもの。ローカル検証用の手順は
`docs_bevy_sample/20260816_google-oauth-client-setup-for-local-gotrue.md` を参照（リダイレクト
URIが `http://localhost:8002/auth/v1/callback` になる点が本ドキュメントとの主な違い）。

**前提の注記**: 本番で Supabase Cloud を使うかセルフホストを続けるかは、
`doc_arch/overview.md` の未決事項としてまだ確定していない。本ドキュメントは
「Supabase Cloud を採用した場合」の手順である。

## 全体の流れ

Google Cloud Console と Supabase ダッシュボードを行き来しながら設定する。順序は
「Supabase側でコールバックURLを確認 → Google側でクライアント作成 → Supabase側に
Client ID/Secretを登録」となる。

## 手順

### 1. Supabase ダッシュボードで Google プロバイダの設定画面を開く

1. [supabase.com](https://supabase.com/dashboard) にログインし、対象プロジェクトを開く。
2. 左メニューの「Authentication」→「Sign In / Providers」を開き、一覧から「Google」を選択する。
3. この画面に **「Callback URL (for OAuth)」** という項目があり、
   `https://<project-ref>.supabase.co/auth/v1/callback` の形式の URL が自動で表示されている。
   これをコピーする（`<project-ref>` はプロジェクトごとに異なるランダムな文字列）。
   この時点ではまだ「Enable Sign in with Google」のトグルはオフのままでよい。

### 2. Google Cloud Console で OAuth 同意画面を設定する

1. [Google Cloud Console](https://console.cloud.google.com/) で、本番用のプロジェクトを
   新規作成する（ローカル検証で使ったテスト用プロジェクトとは分けることを推奨。理由は
   §5「注意事項」参照）。
2. 「APIとサービス」→「OAuth同意画面」を開き、User Type は「外部」を選択して作成する。
3. アプリ名・ユーザーサポートメール・デベロッパーの連絡先情報など、必須項目を入力する。
4. スコープの設定では、Supabase Auth が要求する基本スコープ（`email`・`profile`・`openid`）
   のみで足りる。追加のスコープは今回不要。
5. 「テストユーザー」の追加は本番公開時には必須ではないが、後述の Publishing status を
   「Testing」のままにする場合は、ログインさせたい全員をここに事前登録する必要がある。

### 3. Google Cloud Console で OAuth クライアント ID を発行する

1. 「APIとサービス」→「認証情報」→「認証情報を作成」→「OAuth クライアント ID」を選択する。
2. アプリケーションの種類: 「ウェブ アプリケーション」を選択する。
3. 「承認済みの JavaScript 生成元」に、本番フロントエンドのオリジンを追加する
   （例: `https://your-app.workers.dev`。Cloudflare Workers の本番URL。独自ドメインを
   別途割り当てている場合はそちらも追加する）。
4. 「承認済みのリダイレクト URI」に、手順1でコピーした
   `https://<project-ref>.supabase.co/auth/v1/callback` を追加する。
5. 作成すると「クライアント ID」と「クライアント シークレット」が表示される。この画面を
   閉じると再表示できないため、次の手順に進む前に両方を安全な場所に控えておく
   （`.env` 等、Gitで追跡しないファイルにのみ記録し、本ドキュメントや他のメモには書かない）。

### 4. Supabase ダッシュボードに Client ID / Secret を登録する

1. 手順1の画面（Authentication → Providers → Google）に戻る。
2. 「Enable Sign in with Google」をオンにする。
3. 「Client ID」「Client Secret」の欄に、手順3で発行された値をそれぞれ貼り付けて保存する。

### 5. Site URL / Redirect URLs を本番向けに設定する

1. 「Authentication」→「URL Configuration」を開く。
2. 「Site URL」に本番フロントエンドのURL（例: `https://your-app.workers.dev`）を設定する。
3. 「Redirect URLs」に、ログイン後の戻り先として使う具体的なパスを追加する
   （例: `https://your-app.workers.dev/oauth-sandbox`。`frontend/src/pages/oauth-sandbox`
   の `signInWithOAuth` 呼び出しで指定している `redirectTo` と一致させる）。

### 6. フロントエンドの環境変数を本番向けに設定する

Cloudflare Workers（`worker/wrangler.jsonc` を使ってデプロイするプロジェクト）のビルド時
環境変数として、以下を設定する。ローカル用の `frontend/.env` とは別に、本番のデプロイ設定
（Cloudflare ダッシュボードの環境変数、または CI/CD のシークレット）に登録する。

```
VITE_SUPABASE_URL=https://<project-ref>.supabase.co
VITE_SUPABASE_ANON_KEY=<Supabaseダッシュボードの「API」設定にある anon / public key>
```

`frontend/src/pages/oauth-sandbox` のコード自体は変更不要（環境変数の値を差し替えるだけで
本番の Supabase Cloud に向く）。ローカルの `docker-compose.yml` の `auth`・`auth-gateway`・
`db`・`rest` サービスは本番では使わない。

## 注意事項

- **クライアントシークレットの扱い**: `.env`・Cloudflareのシークレット管理機能にのみ記載し、
  本ドキュメントや他のメモ、コミットメッセージには実際の値を書かないこと。
- **OAuth同意画面の公開状態**: Publishing status が「Testing」のままだと、事前に登録した
  テストユーザー（上限100人）しかログインできない。不特定多数に公開する場合は「アプリを公開」
  操作が必要で、要求スコープの内容によっては Google 側の審査（Verification）が必要になる
  場合がある。審査の要否・所要期間は Google 側の運用に依存するため、公開作業に着手する前に
  [Google の公式ドキュメント](https://support.google.com/cloud/answer/13463073)で執筆時点の
  最新の要件を確認すること（本ドキュメント作成時点では未検証）。
- ローカル検証用のGoogle Cloudプロジェクト（テストユーザー限定）と本番用プロジェクトを
  分けておくと、本番のクライアントシークレットがローカル開発者全員の手元に広がるのを防げる。

## 関連ドキュメント

- `docs_bevy_sample/20260816_google-oauth-client-setup-for-local-gotrue.md`
  （ローカル GoTrue 向けの手順。リダイレクトURIが異なる点以外はほぼ同じ流れ）
- `doc_arch/overview.md`（Supabase Cloud vs セルフホストが未決事項として残っている旨）
- `doc_arch/backend.md` §1・§3（認証にSupabase Authを使う確定方針、自前API層とのポータビリティ方針）
- `doc_arch/frontend.md` §3（フロントが例外的にSupabase Auth SDKを直接使ってよい規定）
- `doc_arch/hosting-and-cicd.md`（Cloudflare Workersへのデプロイ構成）
