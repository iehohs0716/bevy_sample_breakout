import { Link, useParams } from "react-router-dom";
import { BevyGame } from "@/widgets/bevy-game";
import { MOCK_LEVELS } from "@/entities/level";
import "./PlayPage.css";

export function PlayPage() {
  const { levelId } = useParams();
  const level = MOCK_LEVELS.find((level) => level.id === levelId);

  if (!level) {
    return (
      <main className="play-page">
        <p>レベルが見つかりませんでした。</p>
        <Link to="/levels">一覧に戻る</Link>
      </main>
    );
  }

  return (
    <main className="play-page">
      <Link to="/levels">← 一覧に戻る</Link>
      <h1>{level.title}</h1>
      <div className="game-frame">
        <BevyGame
          width={900}
          height={600}
          background={level.background}
          bricks={level.bricks}
          cellSize={level.cellSize}
          brickImage={level.brickImage}
        />
      </div>
    </main>
  );
}
