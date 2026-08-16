# ローカル GoTrue で Google ログインを試すための Google Cloud Console 手順

日付: 2026-08-16

本ドキュメントは、`frontend/src/pages/oauth-sandbox` の動作確認に必要な、Google Cloud Console
側の手動セットアップ手順（ユーザー自身が一度だけ行う作業）をまとめたもの。Google Cloud Console
上の操作は本リポジトリのコードやアーキテクチャ決定そのものではないため、`doc_arch/` ではなく
本フォルダに置く。

## 前提

- ローカルの `auth`（GoTrue）は `api-gw`（Envoy、ポート `8000`）経由で公開される
  （旧`auth-gateway`(nginx)から移行済み。詳細は
  `docs_bevy_sample/20260816_api-gateway-nginx-kong-envoy-tradeoff-analysis.md`）。
  Google 側に登録するリダイレクトURIは **`http://localhost:8000/auth/v1/callback`**。
- 発行した Client ID / Client Secret は、リポジトリルートの `.env`（gitignore 済み）に
  設定する。本ドキュメントには実際の値を書かない。

## 手順

1. [Google Cloud Console](https://console.cloud.google.com/) で新規プロジェクトを作成する
   （または既存プロジェクトを選択する）。
2. 「APIとサービス」→「OAuth同意画面」を開き、User Type は「外部」を選択。
   - Publishing status は「テスト」のままでよい（本番公開前のサンドボックス検証のため）。
   - 「テストユーザー」に、ログイン確認に使う自分の Google アカウントを追加する
     （テスト状態のアプリは登録したテストユーザーしかログインできない）。
3. 「APIとサービス」→「認証情報」→「認証情報を作成」→「OAuth クライアント ID」を選択。
   - アプリケーションの種類: 「ウェブ アプリケーション」
   - 「承認済みのリダイレクト URI」に `http://localhost:8000/auth/v1/callback` を追加。
4. 作成後に表示される「クライアント ID」「クライアント シークレット」を、リポジトリルートの
   `.env` に設定する。

   ```
   GOOGLE_ENABLED=true
   GOOGLE_CLIENT_ID=<発行されたクライアントID>
   GOOGLE_SECRET=<発行されたクライアントシークレット>
   ```

5. `docker compose up -d` で `auth` サービスを再起動し、設定を反映する。
6. `frontend/.env` が無ければ `frontend/.env.example` をコピーして作成し、
   `VITE_SUPABASE_ANON_KEY` にルートの `.env` の `ANON_KEY` と同じ値を設定する
   （`VITE_SUPABASE_URL` は `http://localhost:8000` のままでよい）。
7. `cd frontend && pnpm dev` でフロントエンドを起動し、表示されたURL（例:
   `http://localhost:5173`）の `/oauth-sandbox` にアクセスする。
8. 「Googleでログイン」ボタンを押し、Google の同意画面で手順2で追加したテストユーザーの
   アカウントでログインする。
9. `/oauth-sandbox` に戻ってきたら、email・id（UUID）・provider（`google`）が表示され、
   ログアウトボタンを押すと未ログイン状態（ログインボタン表示）に戻ることを確認する。

## 注意事項

- クライアントシークレットは `.env`（gitignore済み）にのみ記載し、Slack・ドキュメント・
  コミットメッセージ等、本リポジトリ以外もしくは本リポジトリの追跡対象になるファイルには
  絶対に書かないこと。
- テスト状態のまま運用する分にはユーザー数などの上限があるため、本番相当で不特定多数に
  公開する場合は別途「公開」への申請が必要になる（今回のサンドボックス検証の範囲外）。

## 関連ドキュメント

- `doc_arch/backend.md` §1・§3（認証にSupabase Authを使う確定方針、自前API層とのポータビリティ方針）
- `doc_arch/frontend.md` §3（フロントが例外的にSupabase Auth SDKを直接使ってよい規定）
