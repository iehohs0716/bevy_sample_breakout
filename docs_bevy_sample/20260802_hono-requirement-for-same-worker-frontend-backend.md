# 同一Worker構成でもHono移管は強制されるか（調査記録）

日付: 2026-08-02

## 0. 位置づけ

これは「Hono採用を決定した」という記録ではない。フロント・バックエンドを**同一のCloudflare
Worker**に同居させる構成（Workers Static Assetsの`run_worker_first`でパスを振り分ける案）を
検討する過程で、「同一Worker構成を取る場合、Honoフレームワークへの移管が必須になるのではないか」
という懸念が出たため、Cloudflare公式ドキュメント・Hono公式ドキュメントを一次情報として調査し、
この懸念が誤解であることを確認した記録である。Hono採用可否自体は本書の対象外で、未決定のまま。

なお、`doc_arch/backend.md` §3 はフロント・バックエンドを**別のCloudflare Workersプロジェクト**
として分離する方針で確定済みであり、本書の調査結果はこの決定を変更するものではない。今回の
同一Worker案は比較検討の過程で出た代替案であり、その代替案についての疑問点を調査した内容が
以下である。

## 1. run_worker_firstの実装にHonoは必須か

必須ではない。Cloudflare公式ドキュメント
（[Worker script | Cloudflare Workers docs](https://developers.cloudflare.com/workers/static-assets/routing/worker-script/)）
のコード例自体、Honoなど一切使わずプレーンな`fetch(request, env)`のみで完結している。

- `WorkerEntrypoint`の`fetch`メソッドとして実装
- `new URL(request.url)`でパスを判定
- 静的アセットに委譲する場合は`env.ASSETS.fetch(request)`を呼ぶだけ

Honoは「Cloudflare Workers向けフレームワークガイド」の一つ
（[Hono · Cloudflare Workers docs](https://developers.cloudflare.com/workers/framework-guides/web-apps/more-web-frameworks/hono/)）
として案内されているに過ぎない。位置づけは「必須」ではなく「利便性の高いルーティングライブラリ
という選択肢の一つ」である。

## 2. Hono採用が既存のReactフロントエンド（react-router-dom）側に影響するか

影響しない。両者は完全に独立したレイヤー。

- Hono（または`itty-router`等）はサーバー側（Worker）のHTTPルーティング、具体的には
  `/api/*`配下のエンドポイント振り分けを担う。
- `react-router-dom`はブラウザ内のクライアントサイドルーティングを担う。

公式SPAガイド
（[Single Page Application (SPA) | Cloudflare Workers docs](https://developers.cloudflare.com/workers/static-assets/routing/single-page-application/)）
もWorker側のルーティング設定とSPAのクライアントルーティングを別の関心事として扱っており、
Vite設定への影響にも言及がない。

## 3. 「同一Worker案」と「別Worker案」でHono要否に違いが生じるか

違いは生じない。どちらの構成でも、Hono自体はWorker側のAPI実装をどう書くかという任意選択で
あることは変わらない。

同一Worker案の場合、「同じWorker内で`/api/*`だけHono等でルーティングし、それ以外は
`env.ASSETS.fetch()`に委譲する」構成が可能で、フロント資産配信ロジックとAPIロジックが
同居しても、フロントのコード自体は変更不要である。

## 4. Honoを使わない場合の代替

- **`itty-router`**: 超軽量、ミドルウェアなしの最小フットプリント志向。バンドルサイズ最小を
  優先する場合に選ばれる。
- **素の`if`/`switch`分岐によるパス判定**: 公式ドキュメントのデフォルト例そのもの。

Honoはミドルウェア・バリデーション等のエコシステムが充実している分やや重めだが、その分
開発体験は良い。

## Sources

- [Worker script | Cloudflare Workers docs](https://developers.cloudflare.com/workers/static-assets/routing/worker-script/)
- [Single Page Application (SPA) | Cloudflare Workers docs](https://developers.cloudflare.com/workers/static-assets/routing/single-page-application/)
- [Hono · Cloudflare Workers docs](https://developers.cloudflare.com/workers/framework-guides/web-apps/more-web-frameworks/hono/)
- [Getting Started - Cloudflare Workers - Hono](https://hono.dev/docs/getting-started/cloudflare-workers)

## 関連ドキュメント

- `doc_arch/backend.md` §3（フロント・バックエンドを別Workerプロジェクトに分離する決定事項。
  本書はこの決定を変更しない）
- `docs_bevy_sample/20260802_standalone-workers-backend-supabase-dynamodb-crud.md`
  （バックエンドをWorkers単体で新設する場合の実装調査。Honoを「公式推奨のデファクト」として
  採用する前提で書かれているが、これはあくまで別Workerとして新設する場合の実装方式の提案であり、
  本書が示す「Honoは必須ではない」という一般論と矛盾しない）
