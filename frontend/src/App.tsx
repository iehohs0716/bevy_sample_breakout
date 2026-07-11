import { BevyGame } from "./components/BevyGame.tsx";
import "./App.css";

// 背景画像はここ（React 側）から渡す。ゲーム本体(WASM)は 1 ビルドのまま、この URL を
// 差し替えるだけで別背景のブロック崩しを提供できる。同一オリジンの相対パスでも、
// S3 等の外部絶対 URL でも指定可能（外部 URL は配信元の CORS 許可が必要）。
const BACKGROUND_URL = "/assets/backgrounds/sample_sunset.png";

export function App() {
  return (
    <main className="app">
      <h1>Bevy Breakout (WASM)</h1>
      <p className="hint">← / → キーでパドルを操作</p>
      <div className="game-frame">
        <BevyGame width={900} height={600} background={BACKGROUND_URL} />
      </div>
    </main>
  );
}
