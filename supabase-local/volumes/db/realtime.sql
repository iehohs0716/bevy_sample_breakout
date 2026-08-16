-- Source: https://github.com/supabase/supabase/blob/gha/auto-update-js-libs-v2.90.0/docker/volumes/db/realtime.sql
\set pguser `echo "$POSTGRES_USER"`

create schema if not exists _realtime;
alter schema _realtime owner to :pguser;
