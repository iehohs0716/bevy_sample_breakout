// 自前API層(worker/)経由でのユーザーCRUD。doc_arch/backend.md §3の方針により
// Supabaseへの直接アクセス（PostgREST/Storage/Auth管理API）はここでは行わない。

import type { User } from "../model/types";

async function parseJsonOrThrow(response: Response) {
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.error ?? `リクエストに失敗しました (status: ${response.status})`);
  }
  return response.json();
}

export function listUsers(): Promise<User[]> {
  return fetch("/api/users").then(parseJsonOrThrow);
}

export function getUser(id: string): Promise<User> {
  return fetch(`/api/users/${id}`).then(parseJsonOrThrow);
}

// Google OAuthログイン直後に呼ぶ。新規サインアップなら on_auth_user_created
// トリガーが既に profiles を作成済み、既存 auth.users の初回ログインなら
// このリクエストで profiles を作成する（存在すれば何もしない）。
export function ensureProfile(
  id: string,
  input: { email: string; displayName: string },
): Promise<User> {
  return fetch(`/api/users/${id}/profile`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  }).then(parseJsonOrThrow);
}

export function updateUser(id: string, input: { displayName: string }): Promise<User> {
  return fetch(`/api/users/${id}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  }).then(parseJsonOrThrow);
}

export async function deleteUser(id: string): Promise<void> {
  const response = await fetch(`/api/users/${id}`, { method: "DELETE" });
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.error ?? `削除に失敗しました (status: ${response.status})`);
  }
}
