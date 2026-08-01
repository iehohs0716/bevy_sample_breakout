import { Link } from "react-router-dom";
import { MOCK_LEVELS, LevelCard } from "@/entities/level";
import "./LevelListPage.css";

export function LevelListPage() {
  return (
    <div className="level-list-page">
      <Link to="/">トップに戻る</Link>
      <h1>レベル一覧</h1>
      <ul className="level-list">
        {MOCK_LEVELS.map((level) => (
          <LevelCard key={level.id} level={level} />
        ))}
      </ul>
    </div>
  );
}
