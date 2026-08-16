-- Original(fully copied): https://github.com/supabase/supabase/blob/v1.26.08/docker/volumes/db/pooler.sql
\set pguser `echo "$POSTGRES_USER"`

\c _supabase
create schema if not exists _supavisor;
alter schema _supavisor owner to :pguser;
\c postgres
