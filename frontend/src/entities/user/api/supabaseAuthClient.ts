import { createClient } from "@supabase/supabase-js";

/**
 * Supabase Auth 専用のクライアント（doc_arch/frontend.md §3 の例外規定により、
 * ログイン・サインアップのフローに限りフロントから直接 SDK を使う）。
 * DB・Storage へのアクセスはこのクライアントからは行わない
 * （public.profiles の作成は auth.users への INSERT トリガーが担う。
 * worker/drizzle/0001_profile_on_signup_trigger.sql 参照）。
 */
export const supabaseAuthClient = createClient(
  import.meta.env.VITE_SUPABASE_URL,
  import.meta.env.VITE_SUPABASE_ANON_KEY,
  { auth: { flowType: "pkce", detectSessionInUrl: false } },
);
