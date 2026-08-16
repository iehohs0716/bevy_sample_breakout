import { useEffect, useState } from "react";
import type { Session } from "@supabase/supabase-js";
import { supabaseAuthClient } from "../lib/supabaseAuthClient";
import { GoogleLoginButton } from "./GoogleLoginButton";
import "./OAuthSandboxPage.css";

type SandboxStatus = "idle" | "redirecting" | "loggedIn" | "error";

const STATUS_BADGE_LABEL: Record<SandboxStatus, string> = {
  idle: "● 未ログイン",
  redirecting: "◐ リダイレクト中…",
  loggedIn: "● ログイン中",
  error: "● エラー",
};

export function OAuthSandboxPage() {
  const [status, setStatus] = useState<SandboxStatus>("idle");
  const [session, setSession] = useState<Session | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    const { data: subscription } = supabaseAuthClient.auth.onAuthStateChange(
      (_event, nextSession) => {
        setSession(nextSession);
        setStatus(nextSession ? "loggedIn" : "idle");
      },
    );

    const params = new URLSearchParams(window.location.search);
    const code = params.get("code");
    const oauthError = params.get("error_description") ?? params.get("error");

    if (oauthError) {
      setStatus("error");
      setErrorMessage(oauthError);
    } else if (code) {
      // detectSessionInUrl を使わず明示的に交換する（理由は supabaseAuthClient.ts 参照）。
      // 失敗時にエラーを画面へ出すことが目的。
      supabaseAuthClient.auth.exchangeCodeForSession(code).then(({ data, error }) => {
        window.history.replaceState({}, "", window.location.pathname);
        if (error) {
          setStatus("error");
          setErrorMessage(`${error.message} (code: ${error.code ?? "unknown"})`);
        } else if (data.session) {
          setSession(data.session);
          setStatus("loggedIn");
        }
      });
    } else {
      supabaseAuthClient.auth.getSession().then(({ data }) => {
        if (data.session) {
          setSession(data.session);
          setStatus("loggedIn");
        }
      });
    }

    return () => subscription.subscription.unsubscribe();
  }, []);

  async function handleLogin() {
    setStatus("redirecting");
    const { error } = await supabaseAuthClient.auth.signInWithOAuth({
      provider: "google",
      options: { redirectTo: `${window.location.origin}/oauth-sandbox` },
    });
    if (error) {
      setStatus("error");
      setErrorMessage(error.message);
    }
  }

  async function handleLogout() {
    await supabaseAuthClient.auth.signOut();
  }

  return (
    <div className="oauth-sandbox-page">
      <h1 className="oauth-sandbox-page__title">OAuthログイン サンドボックス</h1>
      <p className="oauth-sandbox-page__description">
        Supabase Auth の signInWithOAuth 経由で Google ログインが動作するかを確認するための検証ページ。
      </p>

      <p className={`oauth-sandbox-page__status-badge oauth-sandbox-page__status-badge--${status}`}>
        {STATUS_BADGE_LABEL[status]}
      </p>

      {status === "error" && (
        <p className="oauth-sandbox-page__error">エラー: {errorMessage}</p>
      )}

      {status === "loggedIn" && session ? (
        <div className="oauth-sandbox-page__result">
          <p>ログイン済み</p>
          <dl className="oauth-sandbox-page__user-info">
            <dt>email</dt>
            <dd>{session.user.email ?? "(none)"}</dd>
            <dt>id</dt>
            <dd>{session.user.id}</dd>
            <dt>provider</dt>
            <dd>{session.user.app_metadata.provider ?? "(unknown)"}</dd>
          </dl>
          <button
            type="button"
            className="oauth-sandbox-page__logout-button"
            onClick={handleLogout}
          >
            ログアウト
          </button>
        </div>
      ) : (
        <GoogleLoginButton
          onClick={handleLogin}
          disabled={status === "redirecting"}
          label={status === "redirecting" ? "リダイレクト中…" : undefined}
        />
      )}
    </div>
  );
}
