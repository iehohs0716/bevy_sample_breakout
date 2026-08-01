import type { Level } from "../model/types";
import "./LevelCard.css";

type LevelCardProps = {
  level: Level;
};

export function LevelCard({ level }: LevelCardProps) {
  return (
    <li className="level-card">
      <img
        src={level.thumbnailUrl}
        alt={level.title}
        className="level-card-thumbnail"
      />
      <h2 className="level-card-title">{level.title}</h2>
      <p className="level-card-author">投稿者: {level.author}</p>
      {/* Bevy(winit)は同一ページ内での再初期化に対応していないため、プレイ画面への
          遷移だけはSPA遷移(Link)にせず、必ずフルページロードさせる。 */}
      <a href={`/play/${level.id}`} className="level-card-play-link">
        プレイ
      </a>
    </li>
  );
}
