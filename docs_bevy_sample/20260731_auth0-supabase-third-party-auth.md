# Auth0とSupabaseの認証連携（Third-Party Auth）の仕組み

日付: 2026-07-31

本ドキュメントは、Auth0をIdP（認証基盤）として使いつつDB部分をSupabase(Postgres)にする場合の
連携方式について理解を深めるための解説セッションの記録。**Auth0の採用自体は未決定であり、
本リポジトリの`docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md` §8で検討中の
Auth.js案と並ぶ、あくまで選択肢の一つとして今回扱ったに過ぎない。**

## 0. 前提

本リポジトリは既にSupabase等BaaSへのベンダーロックイン回避方針（自前API層でフロントから
直叩きしない）を持っている
（`docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md` §5.3）。今回はこの方針を
前提に、IdPとしてAuth0を使う場合にSupabase側とどう連携するかを解説した。

## 1. Supabaseの「Third-Party Auth」機能

SupabaseにはAuth0を含む外部IdPが発行したJWTをそのまま信頼できる公式機能（Third-Party Auth）
がある。設定は `supabase/config.toml` に以下のように書く。

```toml
[auth.third_party.auth0]
enabled = true
tenant = "<your-auth0-tenant-id>"
tenant_region = "<region>" # リージョンがある場合
```

この設定により、SupabaseはAuth0のOIDC discoveryエンドポイントからJWKS（公開鍵）を取得し、
Auth0が発行したJWTを検証できるようになる。

重要な点は、この構成では**Supabase自身のAuth(GoTrue)は使わず、Auth0だけが唯一の認証基盤**に
なるということ。SupabaseはAuth0が発行したJWTを受け取って検証するだけの立場になり、
ユーザー管理・パスワード・OAuthフロー自体はすべてAuth0側で完結する。

RLSポリシー内では `auth.jwt()` 関数でAuth0のクレーム（`sub`や`app_metadata`等）を読める。
例えばマルチテナントのRLSであれば以下のような書き方になる。

```sql
create policy "tenant isolation"
  on levels for select
  using (tenant_id = (select auth.jwt() -> 'app_metadata' ->> 'tenant_id'));
```

## 2. この機能が意味を持つ条件

Third-Party AuthとRLSの組み合わせが意味を持つのは、**自前API層がPostgresへRLS経由
（PostgREST等）でアクセスする設計の場合のみ**である。

もし自前API層が `service_role` キー（RLSをバイパスする管理者権限）でPostgresに繋ぎ、
認可判定をAPI層のアプリケーションコード側で行うのであれば、SupabaseにAuth0を
Third-Party Authとして登録する必要はそもそもなく、単に自前API層内でAuth0のJWTを
検証すればよい（Supabase側の設定は不要）。

| 自前API層のPostgresアクセス方式 | Third-Party Auth設定の要否 |
|---|---|
| PostgREST経由・RLSに認可判定を委ねる | 必要（Supabase側がAuth0のJWTを検証できないとRLSの`auth.jwt()`が機能しない） |
| `service_role`で直結・認可判定はAPI層のコードで行う | 不要（Supabase側はAuth0の存在を知らなくてよい） |

本リポジトリの `docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md` §5.3・§10は
既に「認可判定は自前API層のコードで行い、RLSは保険程度に留める」方針であるため、
Auth0を採用する場合は後者（自前API層内でのJWT検証のみ、SupabaseのThird-Party Auth設定は
使わない）の方が既存方針との整合性が高い。

## 3. Enterprise SSO と Social Connection（Google等）はAuth0内では同列

Auth0では、企業向けSSO接続（SAML/OIDC connection）と、Google等のSocial Connectionは、
どちらも「Connection」という同じ枠組みの中の別の入り口にすぎない。

```
[Google]  ─┐
[SAML IdP] ─┼─(Connection)─▶ [Auth0] ──(JWT発行)──▶ [Supabase / 自前API]
[GitHub]  ─┘
```

ユーザーがどちらの経路で認証しても、Auth0は最終的に**同じ形式のJWT**を発行し、
Supabase（や自前API）はそのJWTの発行元がAuth0かどうかしか見ていない。GoogleやSAML先の
IdPとSupabaseが直接やり取りすることはなく、常にAuth0が仲介する。区間で言うと以下の2区間に
分かれ、GoogleとSupabaseが直接会話することはない。

- 区間1: Google/SAML IdP ⇔ Auth0
- 区間2: Auth0 ⇔ Supabase/自前API

## 関連ドキュメント

- `docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md` §5.3・§8・§10（自前API層方針、
  Auth.js検討との並列選択肢としてのAuth0）
- `docs_bevy_sample/20260730_authjs-oauth-on-cloudflare-pages-functions.md`（同じ§12の未決事項
  「認証プロバイダを差し替える可能性」に対する別案の技術調査）
- `docs_bevy_sample/20260730_supabase-react-crud-sso-samples.md`（Supabase標準のSSO構成、
  Supabase Authをそのまま使う場合のsignInWithOAuth）
- `docs_bevy_sample/20260731_supabase-graphql-and-db-only-usage.md`（SupabaseをDB専用として
  使う場合の整理。RLSに依存しない設計との関係）
