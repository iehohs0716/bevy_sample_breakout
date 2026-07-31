# Supabase + React で作る CRUD アプリ（Google/GitHub SSOログイン対応）調査

日付: 2026-07-30

Supabase + React で CRUD アプリを作り、かつ Google/GitHub の SSO ログインに対応させたい場合の
実装パターンを、公式チュートリアル・公式ガイド・GitHub 上の学習用リポジトリ横断で調べた結果。

## 0. 前提条件と本リポジトリの既存方針との関係（重要）

本リポジトリの `doc_arch/web-publish-and-ugc-architecture.md` では、Web公開/UGC基盤の設計として
**「Supabase等のBaaSへのベンダーロックインを避けるため、フロントから Supabase SDK
（PostgREST・RLS・Storage SDK）を直接叩かず、自前API層（Cloudflare Pages Functions想定）に
閉じ込める」**という方針を既に採用している（将来 Neon 等の別Postgresサービスへの移管可能性が
あるため）。

一方、今回調査した以下のサンプル・チュートリアル（公式1件＋GitHub学習用4件、計5件のCRUD実装と
OAuth設定ガイド1件）は、**全て「フロントから Supabase SDK を直接叩く」標準的な構成**であり、
本リポジトリの既存方針とは異なる。

**注意: これらのサンプルはあくまで一般的な Supabase + React CRUD の学習・実装パターンの参考であり、
本リポジトリの `web-publish-and-ugc-architecture.md` で採用した自前API層によるアンチコラプション
レイヤー方針とは異なる（フロント直叩き）構成である。本リポジトリに応用する場合は、CRUDロジック・
RLS設計・OAuthコールバック処理を自前APIエンドポイント側に移植する形で参照すること。**

## 1. 調査サマリー（最重要ポイント）

**今回調べた6件のうち、Google/GitHub OAuth (SSO) をそのまま実装しているサンプルは1件もない。**

- 公式チュートリアル（with-react）: マジックリンク（`signInWithOtp`）のみ。OAuthの言及なし。
- GitHub学習用リポジトリ4件: いずれも認証なし、または旧式のマジックリンク
  （`supabase.auth.signIn` という**廃止済みAPI**）のみ。OAuthコードは皆無。
- Google/GitHub SSOの実装方法自体は、**公式のSocial Loginガイド群**（`signInWithOAuth`）で
  別途調べる必要があり、CRUD部分と認証部分を別々のソースから組み合わせる方針が現実的。

## 2. 比較表

| # | 対象 | 種別 | ビルドツール/UI | 認証方式 | OAuth(Google/GitHub) | Supabaseクライアント配置 | CRUD実装場所 | RLS言及 | 完成度/スター | 最終更新 |
|---|------|------|----------------|----------|----------------------|--------------------------|--------------|---------|---------------|----------|
| 1 | Supabase公式 `with-react` チュートリアル (https://supabase.com/docs/guides/getting-started/tutorials/with-react) | 公式ドキュメント | Vite + React（無地CSS） | マジックリンク（`signInWithOtp`） | なし | `src/supabaseClient.js`（単一ファイル） | `Account.jsx`にコンポーネント直書き | あり（SQL全文公開、`auth.uid() = id`） | 非常に高い | 継続メンテ中 |
| 2 | 公式 Social Login ガイド群 (https://supabase.com/docs/guides/auth/social-login, /auth-google, /auth-github) | 公式ドキュメント | フレームワーク非依存 | OAuth（Google/GitHub含む十数種） | 本命の実装ガイド | 概要のみ | 該当なし | 言及なし | 高い | 継続メンテ中 |
| 3 | `PauloHPMKT/crud-react-supabase` (https://github.com/PauloHPMKT/crud-react-supabase) | 学習用 | Vite + React + TS + MUI | 実装なし | なし | `src/api/createClient.ts`（APIキー**ハードコード**） | `App.tsx`直書き、Readのみ動作。CUDはコメントアウト放置 | なし | 低い | 2024-02（コミット5件） |
| 4 | `normalhuman01/react-supabase-auth-crud` (https://github.com/normalhuman01/react-supabase-auth-crud) | 認証つきCRUD学習用 | Vite + React + react-router-dom | マジックリンクのみ（**廃止済みAPI** `supabase.auth.signIn({email})`） | なし | `src/supabase/client.js` | `TaskContext.jsx` | なし | 低い（AuthContextがuser:null固定のダミー実装、getUserの非同期誤用バグあり） | 2023-12（コミット2件） |
| 5 | `makeuseofcode/React-CRUD-Supabase` (https://github.com/makeuseofcode/React-CRUD-Supabase) | 記事連動 | Create React App | 実装なし（全操作フリー） | なし | `src/utils/SupabaseClient.js` | `App.js`(Create/Read) + `ProductCard.js`(Update/Delete) | なし | 中（★2、各操作後window.location.reload()の力技） | 2023-05 |
| 6 | `Sayli29/supabase-react-crud-app` (https://github.com/Sayli29/supabase-react-crud-app) | 学習用 | Vite + React + MUI | 実装なし | なし | `src/App.jsx`直書き（README記載の環境変数名とコード内`VITE_REACT_URL`が**不一致**） | `InsertDialog.jsx`(Create) + `EditDialog.jsx`(Read/Update/Delete) | なし | 中（READMEは丁寧、UIパターンは一番作り込まれている） | 2023-07（★0） |

## 3. 各サンプル詳細

### 3-1. Supabase公式チュートリアル「Build a User Management App with React」

ソース実体: https://github.com/supabase/supabase/tree/master/examples/user-management/react-user-management

技術スタック: Vite + React。UIライブラリなし。状態管理・ルーティングライブラリなし
（useState/useEffectのみ）。

認証: マジックリンク（パスワードレス）。`Auth.jsx`:

```javascript
const { error } = await supabase.auth.signInWithOtp({ email })
```

Supabase連携:

```javascript
// src/supabaseClient.js
import { createClient } from '@supabase/supabase-js'
const supabaseUrl = import.meta.env.VITE_SUPABASE_URL
const supabasePublishableKey = import.meta.env.VITE_SUPABASE_PUBLISHABLE_KEY
export const supabase = createClient(supabaseUrl, supabasePublishableKey)
```

ルートの `App.jsx` が認証状態を `onAuthStateChange` ＋ `getUser()` で監視し、未ログインなら
`<Auth/>`、ログイン済みなら `<Account/>` を出し分け。CRUD（プロフィールのRead/Update）は
`Account.jsx` に直書き（hooks分離なし）。`upsert` でinsert/updateを兼用。

テーブル設計・RLS（SQL全文公開）:

```sql
create table profiles (
  id uuid references auth.users not null,
  updated_at timestamp with time zone,
  username text unique,
  avatar_url text,
  website text,
  primary key (id),
  unique (username),
  constraint username_length check (char_length(username) >= 3)
);

alter table profiles enable row level security;

create policy "Public profiles are viewable by everyone."
  on profiles for select using (true);

create policy "Users can insert their own profile."
  on profiles for insert with check ((select auth.uid()) = id);

create policy "Users can update own profile."
  on profiles for update using ((select auth.uid()) = id);
```

`profiles.id` を `auth.users` のUUIDに直接紐付け、`auth.uid() = id` で自分の行だけ操作可能に
する設計。この設計はOAuthユーザーでもメール/パスワードユーザーでも同一に機能する
（`auth.users.id` はプロバイダに依らず一意なUUID）ため、OAuthに差し替えても流用可能。

難易度・完成度: 非常に高い。README/トラブルシューティング/ファイル構成図完備。現在の `main`
ブランチは `getUser()` ではなく `getClaims()`（非対称JWT署名鍵対応の新API）に更新されており、
READMEの記載とやや差分がある点は注意。

### 3-2. 公式 Social Login (OAuth) ガイド群

概要: https://supabase.com/docs/guides/auth/social-login
Google: https://supabase.com/docs/guides/auth/social-login/auth-google
GitHub: https://supabase.com/docs/guides/auth/social-login/auth-github

対応プロバイダ: Google, Facebook, Apple, Azure, Twitter, GitHub, GitLab, Bitbucket, Discord,
Figma, Kakao, Keycloak, LinkedIn, Notion, Slack, Spotify, Twitch, WorkOS, Zoom等。

基本コード:

```javascript
// Google
await supabase.auth.signInWithOAuth({ provider: 'google' })

// PKCEフロー（コールバックエンドポイントでコード交換する場合）
await supabase.auth.signInWithOAuth({
  provider: 'google',
  options: { redirectTo: 'http://example.com/auth/callback' },
})

// リフレッシュトークンが必要な場合
await supabase.auth.signInWithOAuth({
  provider: 'google',
  options: { queryParams: { access_type: 'offline', prompt: 'consent' } },
})

// GitHub
async function signInWithGithub() {
  const { data, error } = await supabase.auth.signInWithOAuth({ provider: 'github' })
}
```

コールバック処理: Server-Side Auth（PKCEフロー）を使う場合はコールバックルートで
`supabase.auth.exchangeCodeForSession(code)` を呼んでセッション化する必要がある。純粋な
React SPA構成なら、Supabase JSクライアントがURLフラグメントからトークンを自動検出して
セッションを確立するため、`signInWithOAuth` の呼び出しだけで完結し、`onAuthStateChange` で
セッション変化を監視すればよい。

### 3-3. `PauloHPMKT/crud-react-supabase`

★0 / Fork0 / コミット5件 / 最終push 2024-02-11 / TypeScript

技術スタック: Vite + React + TypeScript + MUI。

認証: 実装なし。

Supabase連携:

```typescript
// src/api/createClient.ts
import { createClient } from "@supabase/supabase-js";
import { supabase_url, supabase_key } from "./config/subaseConfig.json";
const supabase = createClient(supabase_url, supabase_key);
export default supabase;
```

問題点: `src/api/config/subaseConfig.json` に実際のSupabase URL/anon keyが**ハードコードで
コミット**（`.env`未使用）。真似すべきでない実装。

CRUD: `App.tsx` 直書きで `crud-users` テーブルのRead（`select("*")`）のみ動作。Create/Update/
Delete相当は `src/components/teste.tsx` に全行コメントアウトで放置、未完成。

評価: CRUD学習素材としても未完成（実質Readしか動いていない）。

### 3-4. `normalhuman01/react-supabase-auth-crud`

★0 / Fork0 / コミット2件 / 最終push 2023-12-11 / JavaScript / pnpm

技術スタック: Vite + React + react-router-dom（`/`, `/login`, `*`）。READMEなし。

認証: マジックリンクのみ、廃止された旧API使用:

```javascript
// src/context/TaskContext.jsx
const { error } = await supabase.auth.signIn({ email });  // v1時代のAPI。v2ではsignInWithOtpに置換必須
```

認証状態管理の実態:

```javascript
export function AuthProvider({children}) {
    return (
        <AuthContext.Provider value={{ user: null }}>
            {children}
        </AuthContext.Provider>
    )
}
```

`user` が常に `null` 固定でハリボテ（未実装）。`TaskContext.jsx` 内の `getTasks` 等は
`supabase.auth.getUser()` をPromiseとして扱わず誤用しており `user.id` が `undefined` になる
バグの疑いあり。

CRUD: `TaskContext.jsx` に `tasks` テーブルへのCreate/Read/Update/Delete。`userId` カラムで
紐付ける意図はあるが上記バグで実際には機能しない可能性が高い。

評価: 教材としては非推奨（誤ったAPI使用パターンを学んでしまうリスク）。

### 3-5. `makeuseofcode/React-CRUD-Supabase`

★2 / Fork0 / 最終push 2023-05-16 / JavaScript

技術スタック: Create React App。UIライブラリなし。

認証: 実装なし（誰でも全CRUD操作可能）。

Supabase連携:

```javascript
// src/utils/SupabaseClient.js
const supabaseURL = process.env.REACT_APP_SUPABASE_URL;
const supabaseAnonKey = process.env.REACT_APP_SUPABASE_ANON_KEY;
export const supabase = createClient(supabaseURL, supabaseAnonKey);
```

CRAの環境変数プレフィックス `REACT_APP_` を正しく使用。

CRUD: `App.js` にCreate（`products` テーブルへの `insert`）とRead、`ProductCard.js` に
Update/Delete。役割分担は明快。ただし各操作後に `window.location.reload()` でページ全体
再読込する力技。

評価: 認証なし・RLSなしなのでCRUDの「型」だけを見る参考程度。

### 3-6. `Sayli29/supabase-react-crud-app`

★0 / Fork0 / コミット8件 / 最終push 2023-07-20

技術スタック: Vite + React + MUI。README丁寧（Table of Contents、Features、Installation）。

認証: 実装なし。

Supabase連携:

```javascript
// src/App.jsx
const url = import.meta.env.VITE_REACT_URL;
const api = import.meta.env.VITE_REACT_API;
const supabase = createClient(url, api);
```

README内の `.env` 例はCRA向け表記（`REACT_APP_SUPABASE_URL`）だが、実コードはVite変数
`VITE_REACT_URL` を参照しており**READMEとコードの環境変数名が食い違っている**（そのまま
試すと動かない典型的な罠）。

CRUD: `InsertDialog.jsx`（Create、MUI Modal）→`EditDialog.jsx`（Read/Update/Delete、MUI
Table）。`components/Table/Table.jsx` は空ファイル（未使用の残骸）。

評価: 認証なしだがCRUD自体は一番作り込まれている（モーダルUX、テーブル表示）。UIパターンの
参考として有用。

## 4. Google / GitHub OAuthをSupabaseで有効化する設定手順まとめ

### 4-1. Google側の設定（Google Cloud Console）

1. Google Cloud Consoleで対象プロジェクトを用意し、「OAuth同意画面」を設定（スコープに
   `openid` を手動追加。email/profileはデフォルト付与）
2. 「認証情報」→「OAuthクライアントIDを作成」→種類は**Web application**
3. 「承認済みのリダイレクトURI」にSupabaseプロジェクトのコールバックURLを登録
   - 本番: `https://<project-ref>.supabase.co/auth/v1/callback`
   - ローカル（Supabase CLI）: `http://127.0.0.1:54321/auth/v1/callback`
4. 発行された**Client ID**と**Client Secret**を控える

### 4-2. GitHub側の設定（GitHub OAuth App）

1. https://github.com/settings/developers →「New OAuth App」
2. 入力: Application name / Homepage URL / Authorization callback URL（Supabaseのコールバック、
   Google同様の形式）
3. 登録後**Client ID**をコピーし、「Generate a new client secret」で**Client Secret**を発行

### 4-3. Supabase側の設定

1. Dashboard →「Authentication」→「Providers」でGoogle/GitHubを「Enabled」
2. Client ID／Client Secretを貼り付け保存（GoogleでWeb/iOS/Android複数クライアントIDがある
   場合はカンマ区切り、Web用IDを先頭に）
3. 「Authentication」→「URL Configuration」でフロントエンドのリダイレクト先（Site URL /
   Additional Redirect URLs）を登録。ローカル開発用URLは本番リリース前に削除

### 4-4. フロントエンド実装（共通パターン）

```javascript
await supabase.auth.signInWithOAuth({ provider: 'google' })
await supabase.auth.signInWithOAuth({ provider: 'github' })
```

SPA構成ならこれだけで、Supabase JSクライアントがリダイレクト後のURLフラグメントを自動検出して
セッション確立。あとは `onAuthStateChange` ＋ `getUser()` でログイン状態を監視。

サーバーサイドでコード交換が必要な構成（Next.jsのAPI Route等でPKCEフロー使用時）のみ、
コールバックルートで `exchangeCodeForSession(code)` が必要。純粋なReact SPA（Vite）構成で
あれば通常不要。

## 5. 推奨構成（Google/GitHub SSO付きCRUDアプリを作るなら／一般論として）

ベース＝公式 `with-react` チュートリアルの構造＋認証部分だけをSocial Loginガイドの
`signInWithOAuth` に差し替えるのが最も合理的。

1. 認証状態管理とRLS設計は公式チュートリアルのものがそのまま使える。
   `profiles.id references auth.users` ＋ `auth.uid() = id` のRLSポリシーは、ユーザーが
   メール/パスワードで作られようがGoogle/GitHub OAuthで作られようが `auth.users` テーブルに
   同じ形のUUID行が生成されるため無改修で流用可能。
2. `Auth.jsx` 内の `signInWithOtp` 呼び出しを以下に置き換えるだけで完了:
   ```javascript
   <button onClick={() => supabase.auth.signInWithOAuth({ provider: 'google' })}>Googleでログイン</button>
   <button onClick={() => supabase.auth.signInWithOAuth({ provider: 'github' })}>GitHubでログイン</button>
   ```
   `App.jsx` の `onAuthStateChange` ＋ `getUser()`（またはgetClaims）によるセッション監視
   ロジックはそのまま使い回せる。
3. GitHub学習用リポジトリ4件はCRUD部分の「見た目・UIパターン」の参考程度に留める。
   - モーダルUI・テーブル表示のパターンは `Sayli29/supabase-react-crud-app` が一番参考になる。
   - Create/Read/Update/Deleteを素直に別コンポーネントに分ける発想は
     `makeuseofcode/React-CRUD-Supabase` が明快。
   - `PauloHPMKT` と `normalhuman01` は未完成・バグ含みのため反面教師扱い
     （`normalhuman01` の廃止API使用、`PauloHPMKT` のAPIキー直書きは真似NG）。
   - いずれもRLSの記述がないため、RLSポリシーは公式チュートリアルのSQLをベースに自作する
     必要がある。
4. 具体的な組み合わせ手順:
   1. 公式 `react-user-management` のプロジェクト構成（Vite + `supabaseClient.js` +
      `App.jsx`/`Auth.jsx`/`Account.jsx`）をひな形にする。
   2. `Auth.jsx` の中身をGoogle/GitHubの `signInWithOAuth` ボタンに置き換える。
   3. `profiles` テーブルとRLSポリシーは公式SQLをそのまま実行。
   4. CRUD対象を自分のドメインに広げる際は、`Sayli29` のモーダルUIパターンや
      `makeuseofcode` のコンポーネント分割を参考に、`user_id uuid references auth.users` の
      ような所有者カラムを追加し、RLSを `auth.uid() = user_id` で書く。
   5. Google Cloud Console／GitHub OAuth Appの設定は本レポート「4.」の通り進める。

## 6. 本リポジトリへ応用する場合の注意（再掲）

0節の通り、本リポジトリの `web-publish-and-ugc-architecture.md` はフロントからの Supabase SDK
直叩きを避け、自前API層（Cloudflare Pages Functions想定）に閉じ込める方針を取っている。
上記のCRUDロジック・RLS設計・OAuthコールバック処理をそのまま移植するのではなく、

- `supabase.from(...).select/insert/update/delete` 相当の処理は自前APIエンドポイント内に移す
- RLSポリシー（`auth.uid() = id` 等）は自前API側の認可チェックとして再実装する
- `signInWithOAuth` のリダイレクト・コールバック処理も自前APIのルートで受ける

という形で、フロントは自前APIのみを叩く構成に組み替えて参照すること。
