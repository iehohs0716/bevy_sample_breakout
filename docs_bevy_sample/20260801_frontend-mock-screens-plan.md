# フロントエンドのモック画面（Welcome / レベル一覧 / プレイ）追加方針

日付: 2026-08-01

## 1. 位置づけ

現状の`frontend/src/App.tsx`は`BevyGame`コンポーネントを1つだけ直接描画しており、
画面遷移が存在しない。本ドキュメントは、これを

Welcome画面 → レベル一覧画面 → プレイ画面

という画面遷移を持つSPAに拡張するための実装方針である。

**これはモック実装であり、実際のバックエンドAPI・認証とは接続しない。** レベル一覧は
`frontend/src/mock/levels.ts`に定義する静的なモックデータで表示する。将来、実データ取得は
`docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md`で設計された自前API層
（`GET /api/levels`等、Supabase/DynamoDBをフロントから隠蔽するFacade）に差し替える前提の
土台であり、今回はその差し替え先の土台（ルーティング・画面構成）だけを用意する。

## 2. ルーティング構成

`react-router-dom`を新規に依存追加し、次の3ルートを定義する。

| パス | コンポーネント | 内容 |
|---|---|---|
| `/` | `WelcomePage` | ゲームタイトルと簡単な説明、レベル一覧へ遷移するリンクを表示 |
| `/levels` | `LevelListPage` | モックレベルの一覧をカード形式で表示。各カードからプレイ画面へ遷移できる |
| `/play/:levelId` | `PlayPage` | URLパラメータ`levelId`でモックレベル配列から該当レベルを検索して描画 |

`PlayPage`の挙動:

- `levelId`が`MOCK_LEVELS`内に見つかった場合、既存の`BevyGame`コンポーネント
  （`frontend/src/components/BevyGame.tsx`、props: `width` / `height` / `background` /
  `bricks` / `cellSize` / `brickImage` / `onGameClear` / `onGameOver`）に、そのレベルの
  `background` / `bricks` / `cellSize` / `brickImage`を渡して描画する。
- 見つからない場合は「レベルが見つかりませんでした」という文言と、一覧へ戻るリンクを表示する。

## 3. モックレベルのデータ形状

`frontend/src/mock/levels.ts`に以下を定義する。

```ts
type MockLevel = {
  id: string;
  title: string;
  author: string;
  thumbnailUrl: string;
  background: string;
  brickImage?: string;
  bricks: Array<{ x: number; y: number }>;
  cellSize: { width: number; height: number };
};

export const MOCK_LEVELS: MockLevel[] = [ /* 3件、詳細は下表 */ ];
```

各レベルの`thumbnailUrl`は、そのレベルの`background`と同じ画像URLを使う。

| id | title | author | background | brickImage | bricks | cellSize |
|---|---|---|---|---|---|---|
| `robot-pyramid` | ロボットの丘 | (任意) | `/assets/backgrounds/Ameca_robot.jpg` | `/assets/backgrounds/sample_grid.png` | ピラミッド型6段（既存`App.tsx`の`buildPyramidLayout`と同じロジックを流用） | `{ width: 30, height: 30 }` |
| `sunset-default` | 夕焼けの壁 | (任意) | `/assets/backgrounds/sample_sunset.png` | 指定なし | 空配列（`[]`） | `{ width: 50, height: 30 }` |
| `grid-block` | 格子の砦 | (任意) | `/assets/backgrounds/background.png` | `/assets/backgrounds/sample_grid.png` | 中央揃え3行5列の長方形 | `{ width: 50, height: 30 }` |

- `sunset-default`は`bricks`を空配列にする。`BevyGame`側の仕様（`bricks`が空ならBevy側
  デフォルトの敷き詰め配置にフォールバックする、`frontend/src/components/BevyGame.tsx:21`の
  コメント参照）により、これはデフォルト配置のバリエーションを1件見せる意図であって欠損では
  ない。
- `grid-block`のブロック配置は、最上段`y = 150`から始め、行が進むごとに`cellSize.height`分
  下げていく。各行は5列を`cellSize.width`間隔で中央揃えする（`x`は`buildPyramidLayout`の
  中央揃えロジックと同じ考え方で、その行の中心を基準に左右対称配置する）。

## 4. 各画面コンポーネントの配置

`frontend/src/pages/`を新設し、以下を置く。

- `WelcomePage.tsx` / `WelcomePage.css`
- `LevelListPage.tsx` / `LevelListPage.css`
- `PlayPage.tsx` / `PlayPage.css`

## 5. 既存App.tsx / App.cssの扱い

`frontend/src/App.tsx`は、上記3ルートを定義するルーティングの殻（`BrowserRouter`配下に
`Routes`と`Route`を3つ並べるだけ）に置き換える。現状`App.tsx`にある`BevyGame`直接描画・
ピラミッド生成ロジック（`buildPyramidLayout`）は削除し、内容は`PlayPage`側と
`frontend/src/mock/levels.ts`に移動する。

`frontend/src/App.css`の各クラスの移植先:

| クラス | 移植先 | 備考 |
|---|---|---|
| `.app` | 削除（不使用） | ルーティングの殻自体はレイアウトを持たないため不要 |
| `.hint` | `PlayPage.css` | 「← / → キーでパドルを操作」等の操作説明表示に使う |
| `.game-frame` | `PlayPage.css` | そのまま移植 |
| `.game-frame canvas` | `PlayPage.css` | そのまま移植 |
| `#bevy-canvas` | `PlayPage.css` | **削ってはいけないスタイル。** canvasサイズを900x600に固定する。「Bevyの`fit_canvas_to_parent`はこの親サイズに合わせて描画するため、親サイズが未定義だと縮小崩壊する」というコメント（`frontend/src/App.css:15-17`）ごと移植する |

移植後、`frontend/src/App.css`ファイル自体と、`App.tsx`からの`import "./App.css"`は削除する。

## 6. 将来の差し替えポイント

今回は抽象化レイヤーの実装までは行わないが、認識しておくべき将来課題として以下を明記する。

`MOCK_LEVELS`（`frontend/src/mock/levels.ts`）を将来
`docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md`の自前API（`GET /api/levels`
等）に差し替える際、`LevelListPage` / `PlayPage`コンポーネント自体の見た目・表示ロジックは
変えず、データ取得部分（現状は`MOCK_LEVELS`配列を直接参照している箇所）だけを差し替える
必要がある。具体的には、レベル一覧・詳細の取得を`fetch`ベースの非同期処理に置き換え、
画像もURL文字列ではなくAPIから返るレベル定義（`imageUrl`等）経由で取得する形になる見込みだが、
その設計自体は本ドキュメントのスコープ外とする。

## 7. 関連ドキュメント

- `docs_bevy_sample/20260730_web-publish-and-ugc-architecture.md` — 将来のAPI層・データモデル
  （`GET /api/levels`等の契約、Supabase/DynamoDBの隠蔽方針）との関係
