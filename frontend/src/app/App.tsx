import { BrowserRouter, Routes, Route } from "react-router-dom";
import { WelcomePage } from "@/pages/welcome";
import { LevelListPage } from "@/pages/level-list";
import { PlayPage } from "@/pages/play";

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<WelcomePage />} />
        <Route path="/levels" element={<LevelListPage />} />
        <Route path="/play/:levelId" element={<PlayPage />} />
      </Routes>
    </BrowserRouter>
  );
}
