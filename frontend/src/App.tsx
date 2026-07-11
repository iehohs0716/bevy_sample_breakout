import { BevyGame } from "./components/BevyGame.tsx";
import "./App.css";

export function App() {
  return (
    <main className="app">
      <h1>Bevy Breakout (WASM)</h1>
      <p className="hint">← / → キーでパドルを操作</p>
      <div className="game-frame">
        <BevyGame width={900} height={600} />
      </div>
    </main>
  );
}
