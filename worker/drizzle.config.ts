import { defineConfig } from "drizzle-kit";

// drizzle-kit CLI（generate/migrate）はローカル実行時に process.env から読む。
// .dev.vars は wrangler dev 用の変数置き場のため、CLI 実行時は別途 export するか
// `dotenv -e .dev.vars -- drizzle-kit ...` のように読み込ませること。
export default defineConfig({
  dialect: "postgresql",
  schema: "./src/db/schema.ts",
  out: "./drizzle",
  dbCredentials: {
    url: process.env.DATABASE_URL!,
  },
  // auth スキーマは GoTrue が所有するため、drizzle-kit の管理対象を profiles のみに絞る。
  schemaFilter: ["public"],
});
