import { createClient } from "@supabase/supabase-js";

// Supabase Auth の管理用API（auth.admin.*）専用クライアント。
// supabaseUrl はゲートウェイのベースURL（例: http://localhost:8000）のみを渡すこと。
// GOTRUE_API_EXTERNAL_URL のように末尾に /auth/v1 が付いた値を渡すと、
// supabase-js が内部でさらに /auth/v1 を付加し二重パスになる。
export function createSupabaseAdminClient(supabaseUrl: string, serviceRoleKey: string) {
  return createClient(supabaseUrl, serviceRoleKey, {
    auth: { autoRefreshToken: false, persistSession: false },
  });
}

export async function deleteAuthUser(
  supabaseUrl: string,
  serviceRoleKey: string,
  userId: string,
) {
  const admin = createSupabaseAdminClient(supabaseUrl, serviceRoleKey);
  const { error } = await admin.auth.admin.deleteUser(userId);
  if (error) {
    throw new Error(`auth.users の削除に失敗: ${error.message}`);
  }
}
