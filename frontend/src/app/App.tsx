import { BrowserRouter, Routes, Route } from "react-router-dom";
import { WelcomePage } from "@/pages/welcome";
import { LevelListPage } from "@/pages/level-list";
import { PlayPage } from "@/pages/play";
import { OAuthSandboxPage } from "@/pages/oauth-sandbox";
import { UserListPage } from "@/pages/user-list";
import { UserDetailPage } from "@/pages/user-detail";
import { UserEditPage } from "@/pages/user-edit";
import { UserCreatePage } from "@/pages/user-create";

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<WelcomePage />} />
        <Route path="/levels" element={<LevelListPage />} />
        <Route path="/play/:levelId" element={<PlayPage />} />
        <Route path="/oauth-sandbox" element={<OAuthSandboxPage />} />
        <Route path="/users" element={<UserListPage />} />
        <Route path="/users/new" element={<UserCreatePage />} />
        <Route path="/users/:userId" element={<UserDetailPage />} />
        <Route path="/users/:userId/edit" element={<UserEditPage />} />
      </Routes>
    </BrowserRouter>
  );
}
