-- Source: https://github.com/supabase/supabase/blob/gha/auto-update-js-libs-v2.90.0/docker/volumes/db/_supabase.sql
\set pguser `echo "$POSTGRES_USER"`

CREATE DATABASE _supabase WITH OWNER :pguser;
