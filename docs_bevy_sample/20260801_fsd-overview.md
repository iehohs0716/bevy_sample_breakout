# FSD (Feature-Sliced Design) の全体像

日付: 2026-08-01

## 1. 位置づけ

`frontend/`のReactアプリ（Bevy製ブロック崩しのWeb公開用フロント）を、FSD
（Feature-Sliced Design）というアーキテクチャ手法に沿って`src/app/` / `src/pages/` /
`src/widgets/` / `src/entities/`というディレクトリ構成に再構成した。本ドキュメントは
その際に解説したFSDの基本概念（レイヤー・スライス・セグメントの3段構造）を整理したもの。

関連ドキュメント: `docs_bevy_sample/20260801_fsd-dependency-rules.md`（依存方向の詳細）、
`docs_bevy_sample/20260801_fsd-vs-atomic-design.md`（Atomic Designとの比較）。

## 2. FSDの2段階構造（レイヤー→スライス→セグメント）

FSDは「レイヤー」と「スライス」という2段階でディレクトリを整理する手法であり、各スライスの
中身はさらに「セグメント」という3段目に分かれる。

### 2.1 レイヤー（層）

上から下へ6つの層があり、`src/`直下のフォルダに対応する。

```
app
pages
widgets
features
entities
shared
```

### 2.2 スライス

`app`と`shared`以外の各レイヤーの中身を、ビジネス上意味のあるまとまりごとに分割した
フォルダをスライスと呼ぶ。「スライス」はpages専用の言葉ではなく、entities/features/widgets
のどのレイヤーでも同じ意味で使われる。

- `pages/welcome`、`pages/level-list`、`pages/play`はpagesレイヤーの3つのスライス。
- `entities/level`はentitiesレイヤーの1つのスライス。

### 2.3 セグメント

各スライスの中身は、さらに目的別のフォルダ（セグメント）に分かれる。

| セグメント | 役割 |
|---|---|
| `ui` | 見た目（コンポーネント） |
| `model` | 型・状態 |
| `api` | データ取得 |
| `lib` | ロジック |
| `config` | 設定値 |

まとめると、ディレクトリ構造は「レイヤー→スライス→セグメント」の3段構造になる。

## 3. 各レイヤーの役割

- **`pages`**: 特定のURL（ルート）1つに対応する、画面全体の組み立て。widgets/features/
  entitiesを寄せ集めてそのルート専用にレイアウトする。
- **`entities`**: ビジネス上の「モノ」（ドメイン概念）を表すレイヤー。データの型定義・
  それを取得する処理・「1件分を最小限どう表示するか」（例: カード1枚）までを持つ、画面全体
  ではなく部品。特定のページに紐づかず使い回せる。
- **`widgets`**: entities/featuresを複数組み合わせた、大きめの再利用可能なUIブロック。
  特定のURLには紐づかない点はentitiesと同じだが、entitiesより大きな塊（例: ヘッダー、
  サイドバー、複数のカードを並べたグリッド全体）。pagesとentities/featuresの中間に位置する。
- **`shared`**: どのドメインにも依存しない汎用コード（UIキット、ユーティリティ等）。

## 4. セグメントは「あるものだけ作る」

全スライスに`ui`/`model`/`api`/`lib`が必須なわけではない。実際にそのセグメントに入れる
中身ができた時に初めて作る、という運用が前提になる。

## 5. 本リポジトリでの実例

### 5.1 `entities/level/`

以下のセグメントで構成される。

- `model/types.ts` — `Level`型定義
- `api/mockLevels.ts` — `MOCK_LEVELS`というモックデータ配列
- `lib/buildLayout.ts` — `buildPyramidLayout` / `buildCenteredBlockLayout`というブロック
  配置生成ロジック
- `ui/LevelCard.tsx` — 1件分のレベルをカード表示するコンポーネント

### 5.2 `ui`セグメントの後付け（「あるものだけ作る」の実例）

もともとは`entities/level`に`ui`セグメントが存在せず、「1件分のレベルカードの見た目」は
`pages/level-list/ui/LevelListPage.tsx`の`.map()`内に直接JSXで書かれていた。これは
「セグメントはあるものだけ作る」原則により、その時点ではまだ`LevelCard`という独立部品が
存在しなかったための状態だった。

その後、実際に`entities/level/ui/LevelCard.tsx`（と対応する`LevelCard.css`）を切り出し、
`pages/level-list/ui/LevelListPage.tsx`側は

```tsx
MOCK_LEVELS.map(level => <LevelCard key={level.id} level={level} />)
```

という組み立てだけに専念する形にリファクタリングした。これにより「entitiesはモノ
（データ+最小限の見た目）」「pagesはそれを特定のURL用に組み立てた完成品」という役割分担が
実際のコードで実現された。

### 5.3 `widgets/bevy-game/ui/BevyGame.tsx`

Bevy(WASM)を埋め込むコンポーネント。特定のページに紐づかない再利用可能な大きめの部品として
widgetsレイヤーに置かれている。
