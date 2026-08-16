import { createClient } from "@supabase/supabase-js";

/**
 * Supabase Auth 専用のクライアント（doc_arch/frontend.md §3 の例外規定により、
 * ログイン・サインアップのフローに限りフロントから直接 SDK を使う）。
 * DB・Storage へのアクセスはこのクライアントからは行わない。
 *
 * detectSessionInUrl は使わず false にしている。SDK任せの自動検出だと、
 * PKCEのcode交換に失敗した場合もUIに何も表示されず「ボタンのまま」に見えてしまうため、
 * OAuthSandboxPage 側で code を明示的に読み取り exchangeCodeForSession を呼び、
 * 失敗時のエラーを画面に出せるようにする。
 */
export const supabaseAuthClient = createClient(
  import.meta.env.VITE_SUPABASE_URL,
  import.meta.env.VITE_SUPABASE_ANON_KEY,
  { auth: { flowType: "pkce", detectSessionInUrl: false } },
);
