import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { supabaseAuthClient, GoogleLoginButton, ensureProfile } from "@/entities/user";
import "./UserCreatePage.css";

type Status = "idle" | "redirecting" | "exchanging" | "error";

export function UserCreatePage() {
  const navigate = useNavigate();
  const [status, setStatus] = useState<Status>("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const code = params.get("code");
    const oauthError = params.get("error_description") ?? params.get("error");

    if (oauthError) {
      setStatus("error");
      setErrorMessage(oauthError);
      return;
    }
    if (!code) return;

    setStatus("exchanging");
    supabaseAuthClient.auth.exchangeCodeForSession(code).then(async ({ data, error }) => {
      window.history.replaceState({}, "", window.location.pathname);
      if (error || !data.session) {
        setStatus("error");
        setErrorMessage(error?.message ?? "ログインに失敗しました");
        return;
      }

      const { user } = data.session;
      const displayName =
        (user.user_metadata.full_name as string | undefined) ??
        (user.user_metadata.name as string | undefined) ??
        user.email ??
        "";

      try {
        // 新規サインアップなら on_auth_user_created トリガー
        // （worker/drizzle/0001_profile_on_signup_trigger.sql）が既に profiles を
        // 作成済みだが、既存 auth.users の初回ログインではトリガーが発火しないため
        // ここで明示的に ensure（存在すれば何もしない）する。
        await ensureProfile(user.id, { email: user.email ?? "", displayName });
        navigate(`/users/${user.id}`);
      } catch (ensureError) {
        setStatus("error");
        setErrorMessage((ensureError as Error).message);
      }
    });
  }, [navigate]);

  async function handleLogin() {
    setStatus("redirecting");
    const { error } = await supabaseAuthClient.auth.signInWithOAuth({
      provider: "google",
      options: { redirectTo: `${window.location.origin}/users/new` },
    });
    if (error) {
      setStatus("error");
      setErrorMessage(error.message);
    }
  }

  return (
    <div className="user-create-page">
      <Link to="/users">ユーザー一覧に戻る</Link>
      <h1>ユーザー登録</h1>
      <p className="user-create-page__description">
        Googleアカウントでログインすると、そのまま新しいユーザーとして登録されます。
      </p>

      {status === "error" && (
        <p className="user-create-page__error">エラー: {errorMessage}</p>
      )}

      {status === "exchanging" ? (
        <p>登録中…</p>
      ) : (
        <GoogleLoginButton onClick={handleLogin} disabled={status === "redirecting"} />
      )}
    </div>
  );
}
