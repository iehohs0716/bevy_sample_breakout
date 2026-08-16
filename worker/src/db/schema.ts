import { pgSchema, pgTable, text, timestamp, uuid } from "drizzle-orm/pg-core";

// Supabase Auth (GoTrue) が管理する auth.users への外部キー参照のためだけの最小スタブ。
// drizzle-kit はこのテーブル自体を CREATE/ALTER する migration を生成しない
// （auth スキーマは GoTrue が所有するため、id 列の型を揃えて参照するだけに留める）。
const authSchema = pgSchema("auth");
export const authUsers = authSchema.table("users", {
  id: uuid("id").primaryKey(),
});

export const profiles = pgTable("profiles", {
  id: uuid("id")
    .primaryKey()
    .references(() => authUsers.id, { onDelete: "cascade" }),
  email: text("email").notNull(),
  displayName: text("display_name").notNull(),
  createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
  updatedAt: timestamp("updated_at", { withTimezone: true }).notNull().defaultNow(),
});
