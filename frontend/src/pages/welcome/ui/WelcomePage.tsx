import { Link } from "react-router-dom";
import "./WelcomePage.css";

export function WelcomePage() {
  return (
    <div className="welcome-page">
      <h1 className="welcome-page__title">Bevy Breakout</h1>
      <p className="welcome-page__description">
        好きなレベルを選んで、ブロック崩しを遊ぼう。
      </p>
      <Link to="/levels" className="welcome-page__button">
        レベルを選ぶ
      </Link>
      <Link to="/users" className="welcome-page__link">
        ユーザー一覧
      </Link>
      <Link to="/oauth-sandbox" className="welcome-page__link">
        OAuthログイン サンドボックス（検証用）
      </Link>
    </div>
  );
}
