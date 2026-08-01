# CLAUDE.md

Bevy 製ブロック崩し（`game_engine`）を WASM ビルドし、React フロント（`frontend`）から
起動・制御するサンプル。以下の規約に従うこと。

## プロジェクト構成

- `game_engine/` — Bevy ゲーム本体（Rust）。`src/` は次のモジュールに分割。
  - `util`: どのドメイン型にも一切依存しない、純粋に汎用な計算だけを置く（例: アスペクト比を
    保ったまま内接させる `contain_fit`）。ここに置くかどうかは**呼び出し元の数では判断しない**。
    単一ドメインの処理を、呼び出し元が複数あるからという理由だけでここに逃がさないこと
    （呼び出し元が何箇所あっても、中身が単一ドメインの処理なら、そのドメイン名のモジュールに置く）
  - `common`: ドメイン型（`Brick`等）には依存するが、単一の`systems`サブモジュールの中に
    閉じずに複数箇所（`systems::setup`と`systems::update::brick`の両方）から手動で呼び出される
    処理を置く（例: `spawn_brick` / `BrickAssets`）。**`Query`を引数に取る関数（＝実際に
    Bevyのスケジューラが呼ぶsystem）を`common`配下に置くことは禁止する。** `common`はあくまで
    「他のsystemやsetupから普通の関数として手動で呼ばれるヘルパー」の置き場であり、
    Bevyが自動実行するsystem・observerは呼び出し方が全く違う別の役割なので、必ず`systems`側
    （`systems::update`等）に置く。判断基準は「そのドメインの処理を`Query`のような特別な
    system引数を伴わずに関数として直接呼び出せるか」であって、ドメイン型に依存するかどうかは
    `util`と`common`を分けるときの基準（前述）と別軸。
  - `config`: ゲーム全体の定数
  - `components`: Component / Resource / Event の定義
  - `injection`: React(JS) から渡される初期化パラメータの読み取り
  - `systems`: Bevyのスケジューラが実行する全てのsystem・observerへの入り口。実行タイミングで
    3つに分割している。
    - `systems::setup`: `Startup`に一度だけ登録される起動時セットアップsystem
    - `systems::update`: `Update`スケジュールで毎フレーム走るsystemと、`Commands::trigger`で
      発火するobserver（当たり判定は`systems::update::collision`、ブロック単一ドメインの
      system・observerは`systems::update::brick`にさらに分離）
    - `systems::terminate`: 終端状態（`Cleared`/`GameOver`）の`OnEnter`で一度だけ走る、
      フロント(JS)への通知system（通知の実装＝`window.dispatchEvent`等も含む。`main`から
      直接呼ばれるのは`injection`側だけで、通知の実装は`systems`経由でしか使われないため、
      独立トップレベルモジュールにはしない）
- `frontend/` — Vite + React。WASM グルーの読み込みと初期化パラメータ受け渡しを担う。
  アーキテクチャは [Feature-Sliced Design](https://fsd.how/ja/docs/get-started/overview/)（FSD）を
  採用している。詳細は次節「frontend のアーキテクチャ（FSD）」を参照。

## frontend のアーキテクチャ（Feature-Sliced Design）

`frontend/src/` は [Feature-Sliced Design](https://fsd.how/ja/docs/get-started/overview/)（FSD）に
従ってレイヤー分割している。現時点で実際に存在するレイヤーは次の4つ（上位ほど下位に依存できる、
逆方向の依存は禁止）。

- `app/` — アプリ全体のエントリ（ルーティング定義 `App.tsx`、グローバル CSS
  `styles/index.css`）。Vite のエントリポイントである `main.tsx` だけはレイヤーの外
  （`src/` 直下）に置く。
- `pages/` — ルーティング対象となる画面単位のスライス（`welcome` / `level-list` / `play`）。
  各スライスは `ui/` セグメントに実装を置き、`index.ts` で公開 API をエクスポートする。
- `widgets/` — 複数ページから使い得る自己完結した大きな UI ブロック（`bevy-game`。WASM
  化した Bevy を canvas に埋め込むコンポーネント本体）。
- `entities/` — ドメイン上の名詞（`level`）。`model/`（型定義）・`lib/`（そのエンティティに
  閉じた純粋な計算処理）・`api/`（データ取得。現状は実 API が無いためモックデータを返す）に
  分割し、`index.ts` から必要な型・値だけを公開する。

`features/`（ユーザー操作単位の機能）と `shared/`（layer 横断の共通コード）は、現状それらに
該当する処理が発生していないため作成していない。**空のレイヤーを先回りして作らないこと。**
該当する処理（例: レベル投稿・認証などの操作単位の機能、複数エンティティ／widgets から
再利用される汎用 UI 部品）が実際に発生した時点で追加する。

### 依存ルール

- スライス間の参照は必ず各スライスの `index.ts`（公開 API）経由で行い、`ui/` や `model/` 配下
  への深い import（例: `@/entities/level/api/mockLevels`）は禁止する。
- 同一レイヤー内のスライス同士は互いに import しない（例: `pages/play` が
  `pages/level-list` を参照することは禁止）。
- import は相対パスではなく `@/` エイリアス（`tsconfig.json` の `paths` と `vite.config.ts`
  の `resolve.alias` で `src/` に紐付け）を使う（例: `import { MOCK_LEVELS } from "@/entities/level"`）。
- 上記のレイヤー・依存ルールは lint 等では強制していない（`game_engine` 側の設計規約と同様、
  コードレビューで担保する運用）。将来違反が増えるようなら `steiger` 等の FSD 専用 lint の
  導入を検討する。

## ビルド規約（必ずこの流れを踏む）

コードを変更したら「ビルドが通った」で終わらせない。**必ず rust-analyzer によるチェック →
WASM 再ビルド → 実ブラウザ（Playwright）で描画確認**まで行うこと。canvas 描画系の WASM は
「ビルド成功／配信 200」だけでは正常性を保証できない。

### 1. rust-analyzer によるチェック（必須）

Rust コードを変更したら、**まず rust-analyzer で診断を確認する**こと。型・借用・
未使用インポート等の問題を、WASM ビルドを待たずに最速で潰すための必須ステップ。

- エディタ（VS Code の rust-analyzer 拡張等）の診断でエラー／警告が 0 であることを確認する。
- CLI で確認する場合は `cargo check --target wasm32-unknown-unknown`（rust-analyzer と
  同じ `cargo check` ベースの診断）を通すこと。
- 注意: rust-analyzer は一部 **誤検知（false positive）** を出す。既知のものは対処法が
  確立している（例: `Single` の Deref に対する E0614 は `into_inner()` で回避、`Deref`
  派生 Resource への代入は `.0` を使う）。`rustc` / `cargo check` が通るのに rust-analyzer
  だけが赤線を出す場合は誤検知を疑い、既知パターンに沿って回避する。

### 2. WASM 再ビルド

```
cd frontend && pnpm build:wasm
```

内部で `frontend/scripts/build-wasm.sh` が動く:

1. `cargo build --release --target wasm32-unknown-unknown`
2. `wasm-bindgen --target web`（`breakout.js` + `breakout_bg.wasm` を生成）
3. `wasm-opt` があればサイズ最適化（`brew install binaryen` で有効化）
4. `game_engine/assets` を `frontend/public/assets` へコピー

注意:

- `public/wasm` / `public/assets` は gitignore 対象。チェックアウトごとに再ビルドが必要。
- `wasm-bindgen-cli` のバージョンは `Cargo.lock` と一致必須。
- `Cargo.toml` の feature 変更（例: `jpeg`/`webp`）を反映するにも再ビルドが必要。

### 3. Playwright MCP で描画確認

**描画確認は必ず Playwright MCP を使う。** 生の `playwright` を pip / npm で直接入れて
スクリプト実行するのは禁止（Python 環境などを汚すため）。

- 大きな wasm のロード + Bevy 起動 + 描画に時間がかかる。ページ読み込み後に
  **十分（目安 20 秒）待ってから** `#bevy-canvas` のスクリーンショットを取る。
- スクリーンショット + コンソールログ + `window` グローバル（例
  `window.__BREAKOUT_CONFIG__`）の三点で確証を取る。
- 無視してよいノイズ: `favicon.ico` の 404 / AudioContext autoplay 警告 /
  SwiftShader 由来の software rendering WARN / winit の "Using exceptions for control flow"。

## 命名規約（ファイル・関数）

**ファイル名・関数名だけを見て中身が推測できない名前を禁止する。** `common` / `utils` /
`helper` / `misc` のような「何でも置き場」を示唆する名前や、対象・処理内容が省略された曖昧な
名前は、何のための処理かが名前から分からず、後から見た人がファイルを開くまで置き場所の妥当性
すら判断できない。ファイル名・関数名は、それが扱うドメインや処理内容を具体的に表す語にすること。

- どのドメイン型にも依存しない純粋な汎用処理は `util` に置き、関数名自体で処理内容を表す
  （例: `contain_fit`）。
- 呼び出し元が複数あっても、中身が単一ドメインの処理ならそのドメイン名のモジュールに置く。
  置き場所を呼び出し元の数だけで判断しないこと（詳細は「プロジェクト構成」の `util` /
  `common` の説明を参照）。
- 上記の「`common` / `utils` のような曖昧な名前を禁止する」という原則より、本プロジェクトでは
  `util` / `common` という名前自体をユーザーが明示的に選択している（詳細な使い分けは
  「プロジェクト構成」参照）。この 2 モジュール名についてはこの禁止原則の例外として扱い、
  改めて「曖昧だから」という理由だけで改名を提案しないこと。

## エンティティ・コンポーネント設計方針

**役割（セマンティクス）が違うものは、同じエンティティ／コンポーネントで使い回さない。**
1 つの型にフラグや分岐を足して複数の意味を兼ねさせるのではなく、意味ごとに別の型として
分ける。振る舞いの差はマーカーコンポーネントの有無で表現し、`Query` で判別する。

例: アリーナ下端は反射する `Wall` ではなく、専用の `DeathZone` として分離している。

- `Wall`（`components.rs`）: 反射する壁。`WallLocation` は Left / Right / Top のみ。
- `DeathZone`（`components.rs`）: ボールが触れるとライフを減らす下端領域。見た目を持たず
  `Collider` のみ。`Wall` とは別コンポーネントなので `check_for_collisions` 側で
  `Option<&DeathZone>` により反射対象と区別できる。

新しい役割を追加するときは、既存の型に列挙子やフラグを足して兼用させず、この `DeathZone`
と同じように独立した型を切ること。

### ただし「分ける」の対象を取り違えないこと

上の「別の型に分ける」は、**振る舞いを判別するマーカー**（有無を `Option<&T>` / `With<T>`
で見て処理を分岐させるもの。例: `Brick` / `DeathZone`）に対する指針である。**同じ1エンティティに
常に同居し、粒度（カーディナリティ）が同じで、有無の判別にも使われないデータ**まで、フィールド
ごとに別コンポーネントへ割るという意味ではない。それは過剰分割で、クエリに冗長な `Option` が
並ぶだけになる。

同居する不変データは 1 つの型にまとめてよい。**分ける軸は「役割」ではなく「可変性・変更検知」**:

- **spawn 時に確定して以後変わらないデータ**はまとめる。
- **実行中に変化し、`Changed<T>` で対象だけを絞って処理したいものだけ**を独立コンポーネントに切る。

例: ブロックは、不変の `size` / `cell`（格子座標）/ `fill`（塗り方）を `Brick` に集約し、実行中に
変化する `BrokenEdges`（破れた辺）だけを別コンポーネントにして `Changed<BrokenEdges>` による
絞り込み再描画（`redraw_broken_bricks`）を効かせている。「これはブロックだ」という判別は
`With<Brick>` / `Option<&Brick>` で行い、破壊・加点・クリア判定・リセットに使う。
