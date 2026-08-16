-- Source: https://github.com/supabase/supabase/blob/gha/auto-update-js-libs-v2.90.0/docker/volumes/db/logs.sql
-- 注: このプロジェクトではAnalytics/Vectorサービスを導入していないため、
-- 本ファイルが作る_analyticsスキーマは現時点で未使用（詳細はユーザーとの会話ログ参照）。
\set pguser `echo "$POSTGRES_USER"`

\c _supabase
create schema if not exists _analytics;
alter schema _analytics owner to :pguser;
\c postgres
