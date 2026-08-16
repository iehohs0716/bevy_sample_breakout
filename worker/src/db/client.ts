import { drizzle, type NodePgDatabase } from "drizzle-orm/node-postgres";
import { Client } from "pg";
import * as schema from "./schema";

// Cloudflare Workers はリクエストをまたいだ TCP 接続の再利用ができないため、
// リクエストごとに新規 Client を生成する（グローバルにコネクションプールを持たない）。
export async function withDb<T>(
  databaseUrl: string,
  fn: (db: NodePgDatabase<typeof schema>) => Promise<T>,
): Promise<T> {
  const client = new Client({ connectionString: databaseUrl, ssl: false });
  await client.connect();
  try {
    const db = drizzle(client, { schema });
    return await fn(db);
  } finally {
    await client.end();
  }
}
