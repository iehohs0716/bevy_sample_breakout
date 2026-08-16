-- NOTE: change to your own passwords for production environments
\set pgpass `echo "$POSTGRES_PASSWORD"`

ALTER USER authenticator WITH PASSWORD :'pgpass';
ALTER USER pgbouncer WITH PASSWORD :'pgpass';
ALTER USER supabase_auth_admin WITH PASSWORD :'pgpass';
ALTER USER supabase_storage_admin WITH PASSWORD :'pgpass';
-- functions サービス(Edge Runtime)を追加したため、webhooks.sql（本ファイルより先に実行される
-- init-scripts/98-webhooks.sql）が作成する supabase_functions_admin にもパスワードを設定する。
ALTER USER supabase_functions_admin WITH PASSWORD :'pgpass';
