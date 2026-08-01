# Cloudflare Pages Functions から Postgres への TCP 接続サポート状況

日付: 2026-07-31

## 0. 本調査の位置づけ

`docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md` §12 には、以下の未決事項が
残っている。

> Cloudflare Pages Functions から Postgres（Supabase／将来の Neon）への接続方式
> （TCP 直結か、各社提供の HTTP 経由ドライバか）は実装着手時に個別検証が必要（§7）

本ドキュメントはこの未決事項の一部を、**実装着手前に Web 調査ベースで検証したもの**である。
実際にコードを書いて Cloudflare Pages Functions 上で動作確認した「実装検証」ではなく、
公式ドキュメント・公式ブログ等の一次情報を突き合わせた「ドキュメント調査」に留まる点に注意。
§12 の未決事項自体は、この調査だけでは完全には解消しない（§5 参照）。

## 1. Cloudflare Workers の TCP Sockets API（`connect()` / `cloudflare:sockets`）

**わかったこと**

- `cloudflare:sockets` モジュールが提供する `connect()` により、Workers から任意のホストへの
  生 TCP 接続が可能。GA 済みの機能。
- TLS は 2 方式に対応: 接続時点で即座に TLS 化する `secureTransport: "on"` と、平文で接続後に
  `startTls()` を呼んでアップグレードする `secureTransport: "starttls"`（STARTTLS 方式の
  プロトコル向け）。
- 制限事項:
  - グローバルスコープでは `connect()` を呼び出せない（リクエストハンドラ内でのみ生成可能）。
  - Cloudflare 自身の IP レンジ宛の送信 TCP はブロックされる。
  - ポート 25（SMTP）への接続は禁止。
  - インバウンド TCP（Workers 側で listen する用途）は未サポート。
  - 開いたソケットは、そのリクエスト呼び出しにおける同時接続数の上限にカウントされる（§4 参照）。
  - カスタム TLS 証明書（クライアント証明書等）は未対応。

**出典URL**:
- https://developers.cloudflare.com/workers/runtime-apis/tcp-sockets/
- https://blog.cloudflare.com/workers-tcp-socket-api-connect-databases/

## 2. Pages Functions での明示的サポートは公式に未確認（グレー）

**わかったこと**

- Pages Functions 公式の Bindings ページには、対応する Binding の一覧が列挙されているが、
  TCP Sockets API という項目自体が存在しない。TCP Sockets API はそもそも「Binding」という
  性質の機能ではない（`env` 経由で注入されるものではなく、グローバルに import して呼ぶ API）ため、
  この一覧に載っていないこと自体は「非対応の明言」を意味しない。
- Pages Functions は Workers ランタイムの上で動作しており、`compatibility_date` /
  `compatibility_flags`（`nodejs_compat` 等）は Workers と同じ形式で `wrangler.toml`
  （`wrangler pages` 系設定）に書ける。
- ただし「Pages Functions でも TCP Sockets API がそのまま使える」と明記した一次情報は
  見つからなかった。逆に「Pages Functions では使えない」という明記も見つからなかった。
- `connect()` を使った DB 接続系のチュートリアル・アナウンス記事は、調査した範囲では一貫して
  「Workers」という表記のみで書かれており、Pages Functions への言及自体が存在しない。

**出典URL**:
- https://developers.cloudflare.com/pages/functions/bindings/
- https://developers.cloudflare.com/pages/functions/wrangler-configuration/

**未確認・要検証な点**: Pages Functions 上で `cloudflare:sockets` の `connect()` を実際に
呼び出せるか（import エラーにならないか、実行時に拒否されないか）は、公式ドキュメントの
記述だけでは判断できず、実機（`wrangler pages dev` あるいは実デプロイ）での検証が必要。

## 3. Cloudflare Hyperdrive 経由なら明確にサポートあり

**わかったこと**

- Hyperdrive は、Cloudflare のネットワーク内で Postgres（Postgres 互換 DB）・MySQL への
  コネクションプールを維持し、TCP ハンドシェイク・TLS ネゴシエーション・DB 認証の往復
  （合計 7 ラウンドトリップ相当）を削減するマネージドプロキシサービス。
- `node-postgres`（`pg`）・`postgres.js` の prepared statement に対応。
- **Pages Functions での利用は公式ドキュメントに明記されている**。Bindings ページには
  「Hyperdrive bindings allow you to interact with and query any Postgres database from
  within a Pages Function」「Configure Hyperdrive bindings via your Wrangler file the same
  way they are configured with Cloudflare Workers」と記載がある。
- 設定方法は 2 通り: ダッシュボードの Settings > Bindings > Add > Hyperdrive から GUI で
  追加する方法と、`wrangler.toml` に `[[hyperdrive]]` セクションを書く方法（Workers と同一
  形式）。コードからは `context.env.HYPERDRIVE.connectionString` で接続文字列にアクセスする。

**出典URL**:
- https://developers.cloudflare.com/pages/functions/bindings/
- https://developers.cloudflare.com/hyperdrive/get-started/
- https://developers.cloudflare.com/hyperdrive/concepts/how-hyperdrive-works/

## 4. 標準 SQL ドライバでの TCP 直結ガイド

**わかったこと**

- 公式チュートリアル「Connect to a PostgreSQL database with Cloudflare Workers」が存在し、
  `pg`（node-postgres, v8.16.3 以上）を使った接続手順を解説している。
- `nodejs_compat` フラグ（`compatibility_date` が 2024-09-23 以降）の有効化が必須。
- `pg` のメンテナが `connect()` 対応を追加しており、内部的には `pg-cloudflare` という
  シムが `cloudflare:sockets` の `connect()` をラップして、Node.js の `net` / `tls`
  モジュール相当のインターフェースを `pg` に提供している。
- ただし、これらの記述の対象は一貫して明示的に **「Workers」** であり、Pages Functions
  向けの同等チュートリアルは調査した範囲では見当たらなかった。
- 公式の「Connect to databases」ページ自体が、素の TCP 直結はレイテンシ面で不利であると
  述べており、Hyperdrive の利用、またはプロバイダ独自の HTTP 経由ドライバ（Supabase の
  PostgREST／Neon の Serverless Driver／PlanetScale 等）の利用を推奨する立場を取っている。

**出典URL**:
- https://developers.cloudflare.com/workers/tutorials/postgres/
- https://developers.cloudflare.com/workers/databases/connecting-to-databases/

## 5. 既知の制限・注意点

**わかったこと**

- **同時接続数**: Worker（Pages Functions を含むランタイム）の 1 回の呼び出しにつき、
  レスポンスヘッダ待ちの同時接続は 6 本までという制限があり（Free/Paid 共通）、TCP
  ソケットもこのカウント対象に含まれる。2026-04-09 の変更で「レスポンスヘッダを受信済みの
  接続はカウントから除外される」よう緩和されたが、接続確立中の時点では引き続き 6 本制限の
  対象になる。
- Cloudflare 自身の IP レンジへの送信 TCP はブロックされる（§1 と同じ制約）。
- サーバーレス特有の「呼び出しごとに新規接続が必要になり、コネクションプーリングが
  困難」という課題を Cloudflare 自身が認めており、これが Hyperdrive 開発の直接の動機に
  なっている。
- 2026-06-16 には VPC Network bindings 経由での `connect()` 拡張（プライベートなネットワーク
  内の接続先への TCP 到達）がアナウンスされているが、これは Workers の VPC 機能に関する
  話であり、Pages Functions への言及はない。

**出典URL**:
- https://developers.cloudflare.com/changelog/post/2026-04-09-relaxed-connection-limiting/
- https://developers.cloudflare.com/workers/runtime-apis/tcp-sockets/
- https://github.com/cloudflare/cloudflare-docs/issues/21888
- https://blog.cloudflare.com/workers-tcp-socket-api-connect-databases/
- https://developers.cloudflare.com/changelog/post/2026-06-16-tcp-connect-vpc-networks/

## 6. 結論

Pages Functions から Postgres へ **Hyperdrive 経由**で接続することは、公式ドキュメントで
明確にサポートが明記されている。

一方、Hyperdrive を介さず `pg` / `postgres.js` で**素の TCP Sockets API（`connect()`）を
直接叩く**構成については、Pages Functions 上での動作を明示的に保証した一次情報は見つからず、
公式ドキュメントは一貫して「Workers」表記に留まっている。Pages Functions が Workers
ランタイム上で動く以上、技術的に動く可能性は高いと推測されるが、これは推測であり
未確認・グレーな状態にある。

## 7. 本リポジトリへの示唆

`docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md` §5.3 は、自前 API 層
（Cloudflare Pages Functions）が「標準的な Postgres 接続（SQL ドライバ）」でクエリを
発行し、PostgREST の URL 規約や RLS の `auth.uid()` には依存しない、という方針を確定して
いる。この調査結果を踏まえると、その方針を実現する現実的な接続経路は、**素の
`cloudflare:sockets` 直結ではなく、Hyperdrive バインディング経由での Postgres 接続**である
可能性が高い。

Hyperdrive は「接続先の Postgres 互換 DB を指定するだけ」の汎用的な仕組みであり、Supabase・
Neon・AWS RDS/Aurora のいずれに対しても同じ設定方法（`wrangler.toml` の `[[hyperdrive]]`、
またはダッシュボードの Bindings 設定）で使えると読める。そのため Hyperdrive を採用すれば、
§5.3 で懸念していた「プロバイダ固有の HTTP ブリッジ（PostgREST・Neon 専用ドライバ・RDS
Data API）への依存」を避けつつ、同時に「TCP 接続がそもそも Pages Functions で使えるのか」
という実現性の不確実性も解消できる可能性がある。

ただし、この方針も以下の点は実装着手時に個別検証が必要であり、
`web-publish-and-ugc-architecture.md` §12 の未決事項が本ドキュメントだけで完全に解消した
わけではない。

- Hyperdrive 自体の料金体系（無料枠の有無・従量課金の水準）
- Hyperdrive の対応リージョン・地域制約
- Supabase 側の Direct connection（Supavisor を介さない接続）との組み合わせでの実際の
  動作・レイテンシ
- 将来 Neon 等へ移行した場合の Hyperdrive 側設定変更の実際の手間

## 関連ドキュメント

- `docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md` §5.3・§7・§12
  （自前 API 層の接続方式方針、および本調査の元になった未決事項）
- `docs_bevy_sample/20260730_authjs-oauth-on-cloudflare-pages-functions.md` §2・§4
  （同じく Hyperdrive 経由の Postgres 接続、Supabase Direct connection と Supavisor の
  非併用を扱っており、本調査の内容と一致する）
