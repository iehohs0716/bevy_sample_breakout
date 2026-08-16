import { Hono } from "hono";
import { eq } from "drizzle-orm";
import type { Env } from "../env";
import { withDb } from "../db/client";
import { profiles } from "../db/schema";
import { deleteAuthUser } from "../services/supabaseAdmin";

export const usersRoute = new Hono<{ Bindings: Env }>();

usersRoute.get("/", async (c) => {
  const rows = await withDb(c.env.DATABASE_URL, (db) => db.select().from(profiles));
  return c.json(rows);
});

usersRoute.get("/:id", async (c) => {
  const id = c.req.param("id");
  const rows = await withDb(c.env.DATABASE_URL, (db) =>
    db.select().from(profiles).where(eq(profiles.id, id)),
  );
  const profile = rows[0];
  if (!profile) {
    return c.json({ error: "ユーザーが見つかりません" }, 404);
  }
  return c.json(profile);
});

// auth.users の作成自体はフロントから Google OAuth 経由で行う
// （doc_arch/frontend.md §3 の例外規定）。新規サインアップ時の profiles 行は
// on_auth_user_created トリガー（drizzle/0001_profile_on_signup_trigger.sql）が
// 自動生成するが、「登録」ページで既存の auth.users（トリガー導入前に作成済み、
// または一度もこのアプリの登録ページを経由していない既存Googleアカウント）が
// ログインした場合は INSERT が発生せずトリガーが発火しない。
// そのケースを救うため、ログイン直後にフロントから呼ぶ upsert 用エンドポイントを用意する。
usersRoute.put("/:id/profile", async (c) => {
  const id = c.req.param("id");
  const body = await c.req.json<{ email?: string; displayName?: string }>();
  if (!body.email || !body.displayName) {
    return c.json({ error: "email, displayName は必須です" }, 400);
  }

  const rows = await withDb(c.env.DATABASE_URL, (db) =>
    db
      .insert(profiles)
      .values({ id, email: body.email!, displayName: body.displayName! })
      .onConflictDoNothing()
      .returning(),
  );
  if (rows[0]) {
    return c.json(rows[0], 201);
  }
  // 既に存在する場合は既存の行をそのまま返す（表示名の上書きはしない）。
  const existing = await withDb(c.env.DATABASE_URL, (db) =>
    db.select().from(profiles).where(eq(profiles.id, id)),
  );
  return c.json(existing[0]);
});

usersRoute.patch("/:id", async (c) => {
  const id = c.req.param("id");
  const body = await c.req.json<{ displayName?: string }>();
  if (!body.displayName) {
    return c.json({ error: "displayName は必須です" }, 400);
  }

  const rows = await withDb(c.env.DATABASE_URL, (db) =>
    db
      .update(profiles)
      .set({ displayName: body.displayName!, updatedAt: new Date() })
      .where(eq(profiles.id, id))
      .returning(),
  );
  const profile = rows[0];
  if (!profile) {
    return c.json({ error: "ユーザーが見つかりません" }, 404);
  }
  return c.json(profile);
});

usersRoute.delete("/:id", async (c) => {
  const id = c.req.param("id");
  const rows = await withDb(c.env.DATABASE_URL, (db) =>
    db.select({ id: profiles.id }).from(profiles).where(eq(profiles.id, id)),
  );
  if (!rows[0]) {
    return c.json({ error: "ユーザーが見つかりません" }, 404);
  }
  // profiles.id は auth.users.id への ON DELETE CASCADE 外部キーのため、
  // auth.users 側を削除すれば profiles も自動的に連動削除される。
  await deleteAuthUser(c.env.SUPABASE_URL, c.env.SUPABASE_SERVICE_ROLE_KEY, id);
  return c.body(null, 204);
});
