-- Source: https://github.com/supabase/supabase/blob/gha/auto-update-js-libs-v2.90.0/docker/volumes/db/jwt.sql
\set jwt_secret `echo "$JWT_SECRET"`
\set jwt_exp `echo "$JWT_EXP"`

ALTER DATABASE postgres SET "app.settings.jwt_secret" TO :'jwt_secret';
ALTER DATABASE postgres SET "app.settings.jwt_exp" TO :'jwt_exp';
