import { BevyGame } from "./components/BevyGame.tsx";
import "./App.css";

// 背景画像はここ（React 側）から渡す。ゲーム本体(WASM)は 1 ビルドのまま、この URL を
// 差し替えるだけで別背景のブロック崩しを提供できる。同一オリジンの相対パスでも、
// S3 等の外部絶対 URL でも指定可能（外部 URL は配信元の CORS 許可が必要）。
const BACKGROUND_URL = "/assets/backgrounds/Ameca_robot.jpg";
//const BACKGROUND_URL ="https://images.ygoprodeck.com/images/cards_cropped/6983839.jpg";

// 初期ブロック配置もゲーム本体(WASM)を焼き直さず React 側から差し替える。座標は Bevy の
// ワールド座標（中心原点・y 上向き・1 単位 = 1px。アリーナは x∈[-450,450], y∈[-300,300]）で、
// 各ブロックの中心を指定する。ここでは動作確認のため、デフォルトの敷き詰めとは明確に違う
// 「ピラミッド型」を組む。空配列や未指定なら Bevy 側のデフォルト配置にフォールバックする。
const CELL_SIZE = { width: 30, height: 30 };
const BRICK_LAYOUT = buildPyramidLayout();

// ブロックの見た目に使う画像。盤面全体にこの画像を「そのまま」貼ったとみなし、各ブロックは
// 自分の位置に対応する部分だけを切り出して表示する。全ブロックが揃うと 1 枚の絵になり、
// ブロックを壊すと、その穴から背後の背景画像（BACKGROUND_URL）が覗く。
// 配列で複数枚渡すと、ブロック生成順に交互（1 枚目 → 2 枚目 → …）で割り当てられる。
// ここでは背景(sunset)と対比が分かりやすいよう、格子模様(grid)の 1 枚を使う。
const BRICK_IMAGE_URLS = ["/assets/backgrounds/sample_grid.png"];

// 上に頂点を持つピラミッド（最上段 1 個 → 下段ほど増える）を中央揃えで生成する。
function buildPyramidLayout(): Array<{ x: number; y: number }> {
  const rows = 6;
  const colSpacing = CELL_SIZE.width; //セル幅のみ
  const rowSpacing = CELL_SIZE.height; // 同じくセル幅のみ
  const topY = 200; // 最上段の y（アリーナ上部）
  const bricks: Array<{ x: number; y: number }> = [];
  for (let row = 0; row < rows; row++) {
    const count = row + 1; // その段のブロック数
    const y = topY - row * rowSpacing;
    for (let col = 0; col < count; col++) {
      // 段の中央に対して左右対称に配置
      const x = (col - (count - 1) / 2) * colSpacing;
      bricks.push({ x, y });
    }
  }
  return bricks;
}

export function App() {
  return (
    <main className="app">
      <h1>Bevy Breakout (WASM)</h1>
      <p className="hint">← / → キーでパドルを操作</p>
      <div className="game-frame">
        <BevyGame
          width={900}
          height={600}
          background={BACKGROUND_URL}
          bricks={BRICK_LAYOUT}
          cellSize={CELL_SIZE}
          brickImages={BRICK_IMAGE_URLS}
        />
      </div>
    </main>
  );
}
