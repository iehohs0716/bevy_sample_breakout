# Google SSOの実現可能性調査（JWKS自前検証方式との整合性）

日付: 2026-08-02

## 0. 位置づけ

本ドキュメントは「Google SSOを新たに採用することを決定した」というものではない。認証方式として
Supabase Auth（Postgres込み）を採用することは`doc_arch/overview.md`ですでに確定済みであり、
自前APIレイヤー（Cloudflare Workers、`/api/*`、フロントと同一Workerに同居。`doc_arch/backend.md`
§3）がSupabase発行のJWTを`jose`（`createRemoteJWKSet` + `jwtVerify`）でJWKSエンドポイント
（`https://<project_ref>.supabase.co/auth/v1/.well-known/jwks.json`）経由で自前検証する構成も
`docs_bevy_sample/20260802_standalone-workers-backend-supabase-dynamodb-crud.md` §3で調査済みである。

今回はユーザーからの「Google SSOはできるのか」という依頼を受け、この**確定済みアーキテクチャの
上で**Googleソーシャルログインが問題なく実現できるかを、2026年時点の一次情報で確認した。

`docs_bevy_sample/20260730_supabase-react-crud-sso-samples.md`（2026-07-30調査）ですでにGoogle/
GitHub SSOを含む一般的なCRUDサンプルを横断比較済みだが、そこでは「このプロジェクトが選んだ
具体的な認証検証方式（JWKSによる自前JWT検証、非対称鍵RS256）の上でGoogle SSOが成立するか」
という論点までは踏み込んでいなかった。本書はその論点に絞って再確認した点が新規性である。

## 1. Google SSO対応状況

Supabase AuthはGoogleのOAuthログインを現在も公式サポート継続中で、非推奨化の告知は一切ない。

設定手順は以下の通りで、2026-07-30調査時点から変更はない。

- **Google Cloud Console側**: OAuth同意画面を設定 → 「Web application」種別でOAuthクライアント
  IDを作成 → 「承認済みのリダイレクトURI」に`https://<project_ref>.supabase.co/auth/v1/callback`
  を登録する。
- **Supabase側**: Dashboard > Authentication > ProvidersでGoogleを有効化し、Client ID/Secretを
  登録する。

## 2. 非対称鍵とOAuthプロバイダは独立した軸

RS256等の署名鍵方式（`doc_arch/backend.md`が前提とする検証方式）と、ログイン時にどのプロバイダ
（Google等）を使うかは完全に独立している。

- 署名鍵方式（RS256/HS256）は「Supabaseが発行後のJWTをどう署名・検証するか」の話。
- OAuthプロバイダ（Google/GitHub/メール等）は「ユーザーがどの経路で認証されるか」というログイン
  フロー自体の話。

どちらのプロバイダで認証されても、最終的にSupabaseが発行するJWTの構造・署名方式は変わらない
ため、この2つは互いに影響を与えない。

## 3. React SPAでのフロー

`signInWithOAuth({ provider: 'google' })`はPKCEフローに対応しているが、**PKCEはデフォルトでは
有効になっていない**点に注意。使用ライブラリは`@supabase/supabase-js`（内部の認証処理は
`@supabase/auth-js`、旧`@supabase/gotrue-js`の`GoTrueClient`）で、その
[`DEFAULT_OPTIONS`](https://github.com/supabase/supabase-js/blob/master/packages/core/auth-js/src/GoTrueClient.ts)
は次の通り。

```typescript
detectSessionInUrl: true,
flowType: 'implicit',
```

`detectSessionInUrl`はデフォルトtrueだが、`flowType`のデフォルトは`'pkce'`ではなく`'implicit'`。
PKCEフローを使うには`createClient`の`auth`オプションで明示的に指定する必要がある。

```typescript
const supabase = createClient(supabaseUrl, supabaseAnonKey, {
  auth: { flowType: 'pkce', detectSessionInUrl: true },
})
```

この設定をした上でなら、以下の流れがsupabase-js側で自動的に完結する。

```
Google認可 → Supabaseコールバック（/auth/v1/callback） → アプリへのリダイレクト
  → supabase-jsがURLからコード/トークンを自動検出 → セッション確立
```

純粋なReact SPA（サーバーサイドルートなし）では、`exchangeCodeForSession`の追加実装は不要。
これが必要になるのは、Next.js等でサーバー側コールバックルートを自前で受ける構成の場合のみである。

## 4. 自前APIレイヤー（Cloudflare Workers、JWKS検証）への影響

**影響なし。** 自前APIレイヤーは最終的にSupabaseが発行する標準的なJWTをJWKS経由で`jose`検証
するだけであり、認証プロバイダがGoogleであること自体はJWTの構造にも検証方法にも影響を与えない。
Cloudflare Workers側のJWKS検証コード（`docs_bevy_sample/20260802_standalone-workers-backend-supabase-dynamodb-crud.md`
§3）は無改修でよい。

## 5. 結論

現在確定しているアーキテクチャ（Supabase Auth＋自前APIレイヤーでのJWKS検証）の上で、
Google SSOは追加の設計変更なしに実現可能。

## Sources

- [Login with Google | Supabase Docs](https://supabase.com/docs/guides/auth/social-login/auth-google)
- [JavaScript: signInWithOAuth | Supabase Docs](https://supabase.com/docs/reference/javascript/auth-signinwithoauth)
- [JWT Signing Keys | Supabase Docs](https://supabase.com/docs/guides/auth/signing-keys)
- [Social Login | Supabase Docs](https://supabase.com/docs/guides/auth/social-login)

## 関連ドキュメント

- `docs_bevy_sample/20260730_supabase-react-crud-sso-samples.md`（初出のSSO調査。Google/GitHub
  SSOを含む一般的なCRUDサンプルの横断比較）
- `docs_bevy_sample/20260802_standalone-workers-backend-supabase-dynamodb-crud.md`（本書§4で
  参照したJWKS検証方式の調査元）
- `doc_arch/backend.md`（Supabase Auth採用・JWT検証方式の決定事項。本書はこの確定方針の
  実現可能性を確認したものであり、変更しない）
