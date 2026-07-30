# フロントエンド設計

## この文書について

全体像は [overview.md](./overview.md) を参照。バックエンドとの契約（API仕様・データモデル）は
[backend.md](./backend.md) を、機能要件は [requirements.md](./requirements.md) を参照。

## 1. 責務

フロントエンド（`frontend/` React + WASM）が担う役割は以下の通り。

- 静的サイトとして Cloudflare Pages から配信される（詳細は [hosting-and-cicd.md](./hosting-and-cicd.md)）。
- レベル一覧・レベル作成・プレイの 3 画面を提供する。
- 自前 API 層（`/api/levels` 等、詳細は [backend.md](./backend.md)）とのみ通信し、
  `supabase-js` で PostgREST や Storage を直接叩くコードは書かない。例外は認証（§3）のみ。
- 選択したレベルのデータを取得し、既存の `window.__BREAKOUT_CONFIG__` 形式へ変換して
  Bevy（WASM）へ渡す。**この変換はフロントの責務であり、`game_engine` 側の改修は不要。**

## 2. 画面構成

- レベル一覧画面: 自前 API（`GET /api/levels`）から公開レベルのメタ情報
  （タイトル・サムネイル URL・作者等）を取得して表示。未ログインでも閲覧・プレイ可能。
- レベル作成（エディタ）画面: 背景画像・ブロック配置・ブロック画像・タイトルを用意し、
  自前 API（`POST /api/levels`）に送信する新規画面。
- プレイ画面: 既存 `BevyGame.tsx` を流用。

`BevyGame.tsx` 起動前に、選択された `levelId` から自前 API 経由でレベル JSON ＋画像を取得し、
既存の `window.__BREAKOUT_CONFIG__` 形式（`backgroundBytes` / `bricks` / `brickImage` の
バイト列形式）へ変換するアダプタ層を追加する。データ変換の詳細（URL→バイト列化）は
[backend.md](./backend.md) §5.1 のスキーマを参照。

## 3. 認証

ログイン・サインアップのフロー自体は例外的に Supabase Auth の JS SDK を使ってよい
（トークン発行はどの Auth プロバイダを選んでも provider 固有の処理になるため）。
ただし発行された JWT は自前 API 呼び出しの `Authorization` ヘッダに載せるだけで、
それ以外（DB・Storage）の SDK 呼び出しはフロントから行わない（[backend.md](./backend.md) §3 の
ポータビリティ方針）。

※この「例外」自体をなくす案（Auth.js を自前 API 層に内包し、フロントは認証も含めて
自前 API としか話さない構成）を [backend.md](./backend.md) §4 で検討中。採用が決まれば
本節は「ログインも自前 API 層のエンドポイント（Auth.js）を経由する」に置き換える。

## 4. 画像アップロード時のクライアント側検証

サイズ上限・MIME ホワイトリスト（png/jpeg/webp）をアップロード前にクライアント側でも検証する。
ただしこれはユーザー体験向上のための一次チェックであり、正としての検証はサーバー側
（[backend.md](./backend.md) §7 のセキュリティ・認可）で行う。
