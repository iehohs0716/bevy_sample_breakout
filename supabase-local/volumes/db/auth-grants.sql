-- supabase/postgres イメージは auth.uid()/auth.role()/auth.email() を postgres ユーザー
-- 所有で作成する。GoTrue(supabase_auth_admin)は起動時のマイグレーションでこれらの関数を
-- CREATE OR REPLACE しようとするため、所有者を付け替えておかないと
-- "must be owner of function uid" エラーで起動に失敗する。
ALTER FUNCTION auth.uid() OWNER TO supabase_auth_admin;
ALTER FUNCTION auth.role() OWNER TO supabase_auth_admin;
ALTER FUNCTION auth.email() OWNER TO supabase_auth_admin;
