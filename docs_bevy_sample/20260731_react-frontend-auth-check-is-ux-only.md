# Reactのログイン実装とルート/コンポーネントガード、フロント認証チェックの限界

日付: 2026-07-31

本ドキュメントは、React側の認証まわりの実装がどう組まれるか、および「フロントでの
認証チェックはあくまでUXであってセキュリティ境界ではない」という原則について理解を
深めるための解説セッションの記録。

## 1. Reactでのログイン実装の実態

React側は認証UI（パスワード欄・Googleボタン等の入力フォーム）を自前で持たない。
ログインボタンはAuth0（や同種のSDKを提供するIdP）がホストする認証ページへ丸ごと
リダイレクトするだけである。

流れは以下の通り。

1. Reactのログインボタンから、Auth0がホストする認証ページへリダイレクト
2. ユーザーがAuth0側の画面でメール/パスワードやGoogleログイン等を選択して認証
3. 認証完了後、Auth0はReactアプリのコールバックURLにユーザーを戻す
4. 認可コードをJWTに交換するやり取りはAuth0 SDK（`@auth0/auth0-react`等）が裏で自動的に行う
5. 以降Reactはこのトークンをメモリに保持し、自前APIへのリクエストのたびに
   `Authorization: Bearer <JWT>` ヘッダーとして付与する

認証ロジック自体（パスワードチェック・Google連携・SSO/SAMLのやり取り）はReactのコードには
一切現れない。これはAuth0に限らず、Auth.jsやSupabase AuthのSDKを使う場合も同様の構造になる
（`docs_bevy_sample/20260731_auth0-supabase-third-party-auth.md`参照）。

## 2. ページ単位の出し分け（ルートガード）

ページ全体をログイン済みユーザーだけに見せたい場合は、ルーティングの階層でガードする。

```jsx
function ProtectedRoute({ children }) {
  const { isAuthenticated, isLoading } = useAuth0();
  if (isLoading) return <Loading />;
  if (!isAuthenticated) return <Navigate to="/login" />;
  return children;
}

<Route path="/mypage" element={<ProtectedRoute><MyPage /></ProtectedRoute>} />
```

## 3. 画面内の一部だけの出し分け（コンポーネント単位）

ページ自体は誰でも見られるが、その中の一部だけを会員限定にしたい場合はコンポーネント内部で
分岐する。

```jsx
function Page() {
  const { isAuthenticated } = useAuth0();
  return (
    <div>
      <PublicContent />
      {isAuthenticated ? <MemberOnlyContent /> : <LoginPrompt />}
    </div>
  );
}
```

2節・3節は**同じ`isAuthenticated`という一つの値を、ルート単位で使うかコンポーネント内部で
使うかという粒度の違いだけ**であり、別の仕組みではない。

## 4. 最重要の注意点: フロントのチェックはUXであってセキュリティ境界ではない

上記2・3はいずれも**UX（見た目の出し分け）に過ぎず、セキュリティ境界ではない**。

ブラウザ上で動くJavaScriptはユーザー側で改変・迂回が可能である。

- 開発者ツールでコードを書き換えて`isAuthenticated`を強制的に`true`扱いにする
- Reactを経由せず、自前APIのエンドポイントに直接`curl`/Postman等でリクエストを送る

このため、フロントの`isAuthenticated`チェックだけでは実データも非公開ファイルも守れない。

**本当の防御線は必ずサーバー側（自前API層）にあり、リクエストのたびにJWTを検証してから
初めてデータ／ファイルの中身を返す**、という実装だけが唯一の実効的な保護になる。

ファイル配信も同様の注意が必要になる。静的に誰でも取得可能な場所（公開S3バケット等）に
非公開ファイルを置いてしまうと、フロント側でリンクを画面に表示していなくても、URLさえ
分かれば直接叩いて取得できてしまう。そのため、非公開ファイルは必ずJWT検証を通る
サーバーコード経由でのみ配信する設計にする必要がある。

この原則は、本リポジトリの`docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md`
§10（認可判定は自前API層のコードで行う。投稿者本人のみ書き込み許可というルールをAPI層に
実装する）とも整合する。フロント側のガード（本ドキュメント2・3節）はあくまで「見た目上
迷わせない」ための補助であり、実際のデータ保護はAPI層側のJWT検証・認可チェックが担う。

## 5. 具体例: Cloudflare Pages FunctionsでのJWT検証実装

自前API層の実装先として本リポジトリで想定している **Cloudflare Pages Functions**
（Cloudflare Pagesの静的サイトに、ファイル配置だけでサーバー処理を追加できる機能。
`functions/api/levels.ts` を置くと `/api/levels` というエンドポイントになる）上での、
4節で述べた「サーバー側のJWT検証」の具体的な実装例。

Cloudflare Pages FunctionsはNode.jsではなくCloudflare Workersランタイム（V8 isolate）上で
動くが、JWT検証に必要な `fetch`（JWKS取得）と `crypto.subtle`（WebCrypto、署名検証）は
どちらもWorkersの標準APIとして提供されているため、DBへのTCP直結（設計書§12の未検証事項）とは
異なり実装のハードルは低い。ライブラリは `jose`（Cloudflare Workers対応を明記したJWT/JOSE
ライブラリ）が定番。

**サーバー側（`functions/api/levels.ts`）:**

```typescript
import { jwtVerify, createRemoteJWKSet } from 'jose';

// Auth0の公開鍵一覧を取りに行く場所。内部で自動キャッシュされる
const JWKS = createRemoteJWKSet(
  new URL('https://YOUR_TENANT.auth0.com/.well-known/jwks.json')
);

export async function onRequestPost(context) {
  const { request } = context;

  const authHeader = request.headers.get('Authorization');
  if (!authHeader?.startsWith('Bearer ')) {
    return new Response('Unauthorized', { status: 401 }); // トークンが無ければ即拒否
  }
  const token = authHeader.slice('Bearer '.length);

  try {
    const { payload } = await jwtVerify(token, JWKS, {
      issuer: 'https://YOUR_TENANT.auth0.com/',
      audience: 'YOUR_API_IDENTIFIER',
    });

    const userId = payload.sub; // 検証OK。ログイン中のユーザーID
    const body = await request.json();
    // ここでDBへの保存処理などを行う

    return new Response(JSON.stringify({ ok: true, userId }), { status: 200 });
  } catch {
    // 署名不正・期限切れ・発行者不一致は全部ここに落ちる
    return new Response('Invalid token', { status: 401 });
  }
}
```

**Reactアプリ側:**

```jsx
const { getAccessTokenSilently } = useAuth0();

async function postLevel(levelData) {
  const token = await getAccessTokenSilently();

  const res = await fetch('/api/levels', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify(levelData),
  });
}
```

### 「Reactアプリからしか叩けないようにする」ことはできない

上記のサーバー側コードは、リクエストがReactアプリ・curl・Postmanのどれから来たかを
一切区別していない。見ているのは「有効なJWTが付いているか」だけである。これは意図的で、
そもそも**「呼び出し元のアプリを限定する」という制御はHTTP APIには存在しない**。フロントの
JSバンドルに埋め込んだ秘密情報は、フロントを直接叩く誰からも読める（本ドキュメント4節と
同じ理由）ため、「Reactアプリだけの合言葉」は原理的に作れない。

代わりに設計として成立させるべきは「誰から来たか」ではなく「行為ごとに、その行為をして
よい人か（＝有効なJWTを持つ本人か）」の判定であり、これは
`docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md` §10の認可方針そのものである。

## 関連ドキュメント

- `docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md` §10（認可判定は自前API層の
  コードで行う方針）・§12（Cloudflare Pages FunctionsからPostgresへのTCP接続方式は別途検証が
  必要という未決事項）
- `docs_bevy_sample/20260731_auth0-supabase-third-party-auth.md`（Auth0が発行するJWTと、
  自前API層でのJWT検証の関係）
