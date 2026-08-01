# Cloudflare Pages / Workers への frontend 実デプロイ作業記録

日付: 2026-08-01

事前調査 [[20260801_cloudflare-pages-deploy-considerations]] を踏まえ、実際に `frontend` を
Cloudflare にデプロイした作業記録。事前調査は「Cloudflare Pages の Git 連携ビルド機能を使う」
前提で Rust ツールチェイン導入の是非等を検討していたが、実際にはその前提自体を採用しなかった。
本書は「事前調査のどこが理論通りで、どこが実際にやってみて初めて分かったか」の対比を主目的とする。

## 1. 採用した方針（事前調査からの転換）

事前調査では次の2案を提示していた。

1. Cloudflare Pages 側のビルド環境に Rust ツールチェインを導入する（非公式サポート）。
2. wasm 成果物を git 管理下に置き、Cloudflare Pages 上では `vite build` だけ実行する。

ユーザーの判断により、いずれも不採用。**wasm 成果物は一切 git にコミットせず、ローカルで
ビルドした `dist` を Wrangler CLI で直接アップロードする**方針を採った。これにより Cloudflare
側の Git 連携ビルド機能自体を使わないため、事前調査の最大の懸念だった「Rust が Cloudflare Pages
公式サポート言語に無い」問題（[[20260801_cloudflare-pages-deploy-considerations]] の §2）は
そもそも発生しなくなった。

## 2. 事前調査との対比

| 事前調査での懸念・記載 | 実際にやってみてわかったこと |
|---|---|
| §2 Rust ビルド環境が無い問題 | ローカルビルド＋Wrangler直アップロード方式を採ったため、そもそも問題自体が発生しなかった |
| §1 `404.html` が無ければ自動SPAフォールバック（未検証） | Cloudflare Pages では実機（curl）で確認、事実通りだった。ただし Workers Static Assets 移行後はこの自動フォールバックが**効かない**という、Pages/Workersの差異が新たに判明（§5参照） |
| §4 `.wasm` のMIMEタイプは未確認（`_headers`での明示を推奨） | 実機確認の結果、Cloudflare Pages・Workers Static Assets いずれもデフォルトで `Content-Type: application/wasm` を正しく返した。`_headers` での明示は不要だった |
| §5 モノレポでの Root Directory 設定（状況証拠ベース） | 今回の方式では Cloudflare 側ビルドを使わないため、この論点自体が無関係になった |
| （事前調査では言及なし） | `wasm-opt` 未導入による wasm 未最適化（25.3MiB）でサイズ上限超過、および `wasm-opt` 導入後の feature フラグ不足バグという、事前調査時点では存在を知り得なかった `build-wasm.sh` 側の実装バグが発覚した（§4参照） |

## 3. 実際の作業手順（時系列）

1. Wrangler 導入: `frontend` 配下で `pnpm add -D wrangler`（devDependency、v4.118.0）。
2. Cloudflare へのログイン: `pnpm exec wrangler login` はブラウザ経由の OAuth 認証のため
   エージェントは代行できず、ユーザー自身に実行してもらった。
3. Pages プロジェクト作成: `wrangler pages deploy dist --project-name=bevy-breakout` を
   いきなり実行すると `The Pages project "bevy-breakout" does not exist` エラー。
   `wrangler pages project create bevy-breakout --production-branch=master` で明示的に
   プロジェクトを作成する必要があった。
4. 最初のデプロイ失敗（ファイルサイズ超過）: `wrangler pages deploy dist --project-name=bevy-breakout`
   が `Error: Pages only supports files up to 25 MiB in size / wasm/breakout_bg.wasm is 25.3 MiB
   in size` で失敗。開発機に `wasm-opt`（binaryen）が入っておらず、
   `frontend/scripts/build-wasm.sh` 内の「binaryen があれば `wasm-opt -Oz` で最適化する」分岐が
   これまで一度も実行されていなかったことが原因（未最適化のまま25.3MiB）。
5. `brew install binaryen` で導入。
6. `build-wasm.sh` のバグ発覚と修正: binaryen 導入後に `pnpm build:wasm` を実行すると、
   `wasm-opt` が `[wasm-validator error] memory.copy operations require bulk memory operations`、
   続けて `i32.trunc_sat_f64_u ... unexpected false: all used features should be allowed` という
   validation エラーで落ちた。rustc（`wasm32-unknown-unknown`）が出力する wasm は
   bulk-memory 命令や nontrapping-float-to-int 命令を使うが、`wasm-opt` はデフォルトでこれらの
   機能を無効化した状態で入力を検証するため。個別に `--enable-bulk-memory` を足しても別の
   未許可命令で次々失敗したため、最終的に次のように修正した。

   ```
   # 修正前
   wasm-opt -Oz -o "$OUT_DIR/wasm/${OUT_NAME}_bg.wasm" "$OUT_DIR/wasm/${OUT_NAME}_bg.wasm"
   # 修正後
   wasm-opt -Oz --all-features -o "$OUT_DIR/wasm/${OUT_NAME}_bg.wasm" "$OUT_DIR/wasm/${OUT_NAME}_bg.wasm"
   ```

   個別の機能フラグを積み上げるより `--all-features` で一括許可する方が、今後 rustc の出力が
   別の機能を使うようになっても壊れにくく堅牢。
7. 再ビルドで解決確認: 修正後 `pnpm build:wasm` で `breakout_bg.wasm` は 25.3MiB→17MiB に縮小し、
   25MiB 上限内に収まった。
8. デプロイ成功: `vite build` → `wrangler pages deploy dist --project-name=bevy-breakout
   --commit-dirty=true` で成功。デプロイURL（例: `https://78e2b427.bevy-breakout-5cc.pages.dev`）
   が発行された。
9. 実機での動作確認（curl。今回は Playwright MCP でのブラウザ確認はスキップ）。
   - `GET /` → 200
   - `GET /play/level-1`（React Router の動的ルート、実ファイルとしては存在しない）→ 200。
     事前調査 §1 の「`404.html` が無ければ自動でSPAフォールバックする」という公式記載が、
     実際のデプロイでも機能することを確認できた。
   - `HEAD /wasm/breakout_bg.wasm` → `content-type: application/wasm`。事前調査 §4 で
     「未確認」としていた点が、実機では正しく返ることを確認できた。
10. `package.json` にデプロイ用スクリプトを追加。最終的にスクリプト名は `deploy` ではなく
    `build-deploy`（ユーザー指名）とした。
11. `frontend/wrangler.jsonc` を新規作成し、プロジェクト名・出力先を CLI オプションではなく
    設定ファイルに明示した。

    ```jsonc
    {
      "$schema": "node_modules/wrangler/config-schema.json",
      "name": "bevy-breakout",
      "pages_build_output_dir": "dist"
    }
    ```

    これにより `wrangler pages deploy` を引数無しで実行してもプロジェクト名・出力ディレクトリを
    自動認識することを実機確認した。`build-deploy` スクリプトも
    `"pnpm build && wrangler pages deploy"`（`--project-name` 指定不要）に簡略化した。

## 4. 副産物: `build-wasm.sh` の恒久バグ

binaryen（`wasm-opt`）を導入している開発者がこれまで誰もいなかったため、「`wasm-opt` に
bulk-memory 等の機能フラグを渡していない」というバグが長期間気づかれずに残っていた。このバグは
Cloudflare へのデプロイ作業（＝ファイルサイズ上限に引っかかったこと）が無ければ、今後も
気づかれなかった可能性が高い。修正箇所: `frontend/scripts/build-wasm.sh` の `wasm-opt` 呼び出し
（`--all-features` 追加、§3手順6参照）。

## 5. 追記: Cloudflare Pages から Workers (Static Assets) への移行（同日中に発生）

ユーザーから「Cloudflare Pages は廃止の方向で Workers (Static Assets) に統合されていくらしい」
という外部情報を受け、公式ドキュメント・移行ガイドを調査した。

- 確認結果: Pages が正式に非推奨と明記されているわけではないが、新機能は Workers 優先で
  追加されており（Durable Objects, Cron Triggers, Gradual Deployments, Workers Logs 等は
  Pages に無い）、公式に新規プロジェクトは Workers を推奨する移行ガイドが用意されている。
  実際 `wrangler pages project create` 実行時にも「Workers are the recommended way to deploy
  all new projects」という警告が出ていた。
- 本プロジェクトは Pages Functions 等の動的機能を使わない純粋な静的サイトのため、移行は軽微
  だった。
- **重要な差分**: Cloudflare Pages はデフォルトで SPA フォールバックが効く（`404.html` 非設置時、
  事前調査 §1・本書§3手順9参照）が、**Workers Static Assets ではこの自動フォールバックは無く、
  `wrangler.jsonc` の `assets.not_found_handling: "single-page-application"` を明示的に
  設定する必要がある**。設定を忘れると、Pages では動いていた SPA ルーティングが Workers 移行後に
  壊れる（`/play/:levelId` 等の直接アクセスが404になる）ので要注意。
- 移行後の `wrangler.jsonc`:

  ```jsonc
  {
    "$schema": "node_modules/wrangler/config-schema.json",
    "name": "bevy-breakout",
    "compatibility_date": "2026-08-01",
    "assets": {
      "directory": "dist",
      "not_found_handling": "single-page-application"
    }
  }
  ```

- `package.json` の `build-deploy` スクリプトも `"pnpm build && wrangler deploy"`（
  `wrangler pages deploy` ではなく `wrangler deploy`）に変更。
- デプロイ後、実機確認（curl）で以下を確認。
  - `/`, `/levels`, `/play/level-1` いずれも200
  - `/index.html` を直指定すると307で`/`にリダイレクト（正規URLへの整理。Workers Static
    Assets のデフォルト挙動）
  - `breakout_bg.wasm` の Content-Type は `application/wasm`
  - **注意点（ハマりどころ）**: `wrangler deploy` 直後の数秒間は Cloudflare のエッジへの伝播が
    完了しておらず、`/` や `/levels` が一時的に404を返すことがあった。デプロイ直後に動作確認
    する際は、数秒待ってから再確認すること。
- デプロイURL: `https://bevy-breakout.koma4024.workers.dev`
- 出典:
  - https://developers.cloudflare.com/workers/static-assets/migration-guides/migrate-from-pages/
  - https://developers.cloudflare.com/workers/static-assets/

## 6. 結論・現状のデプロイ手順

`frontend/` で `pnpm build-deploy` を実行するだけで、ローカルビルド（wasm ビルド含む）→
Cloudflare Workers (Static Assets) へのデプロイが完結する。Cloudflare 側の Git 連携ビルド機能は
使わない。事前に一度だけ `pnpm exec wrangler login` でのログインが必要（既に完了済み）。

## 7. 変更ファイル一覧（2026-08-01時点、この時点ではまだ git commit していない）

- `frontend/package.json`（wrangler devDependency 追加、`build-deploy` スクリプト追加。その後
  Workers 移行に伴い内容変更）
- `frontend/pnpm-lock.yaml`
- `frontend/scripts/build-wasm.sh:` `wasm-opt` 呼び出しに `--all-features` を追加
- `frontend/wrangler.jsonc`（新規ファイル。Pages 形式から Workers Static Assets 形式に変更済み）

## 関連ドキュメント

- [[20260801_cloudflare-pages-deploy-considerations]]（本書の元になった事前調査。SPAフォールバック・
  Rustビルド環境・WASM配信MIMEタイプ・モノレポRoot Directory設定を一次情報ベースで検討した記録）
