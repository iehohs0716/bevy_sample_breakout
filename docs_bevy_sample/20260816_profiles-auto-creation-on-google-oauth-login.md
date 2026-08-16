# Google OAuthログインをトリガーにした`public.profiles`自動生成の設計・実装・トラブルシューティング

作成日: 2026-08-16

## 0. 位置づけ

ユーザー登録機能（Supabase Auth連携 + `public.profiles`のCRUD管理画面）の実装の一部として、
「Google OAuthでログインしたら、追加のフォームなしで`public.profiles`にユーザー行が
自動的に作られる」仕組みを実装した経緯を記録する。`doc_arch/backend.md` §3（自前API層で
Supabaseを隠蔽するポータビリティ方針）・`doc_arch/frontend.md` §3（認証フローに限りフロントが
Supabase Auth JS SDKを直接使ってよい例外規定）を前提とする。

ローカルGoTrue自体のセットアップ・CORS周りのトラブルシューティングは別記録
（`docs_bevy_sample/20260816_local-gotrue-google-login-troubleshooting.md`）を参照。本書は
「ログイン後に`profiles`行をどう作るか」というアプリケーションロジック側の設計に焦点を当てる。

## 1. 方針転換の経緯

当初は`worker/`（自前API層）に、email/passwordを直接指定して`auth.admin.createUser`
（Supabase Admin API）経由でユーザーを作るPOSTエンドポイントを実装した。しかしユーザーから
「ユーザーの作成自体はGoogle Authにしたいな、いちいちemailを入力するのは手間だろう」という
指摘があり、方針を変更した。

ヒアリングの結果、`public.profiles`行の作成方式は「Postgresトリガーで自動作成する」
（Supabase公式の標準パターン）を採用することに決まった。email/password版のPOSTエンドポイントは
削除し、以降はGoogle OAuthログインのみをユーザー作成の入口とする。

## 2. 仕組みの全体像

ログイン成功から`profiles`行が確実に存在する状態になるまでの流れは、次の4段階で構成される。

```
[1] フロント                [2] GoTrue               [3] Postgres              [4] フロント
signInWithOAuth   ------>   Google側で認可   ------>  （GoTrueが直接書く）
(Google)                    ↓ callback                auth.users に INSERT
                             exchangeCodeForSession                 |
                             （PKCEコード交換）                       | AFTER INSERT トリガー発火
                                                        public.profiles に自動INSERT
                                                       （新規ユーザーの場合のみ）
                                                                    |
                                                                    v
                                                        ensureProfile (PUT /api/users/:id/profile)
                                                        ON CONFLICT DO NOTHING で upsert相当
                                                        → 既存ユーザーの初回ログインでも
                                                          profilesの存在を保証
```

1. **フロント**: `frontend/src/pages/user-create/ui/UserCreatePage.tsx`の`handleLogin`が
   `supabaseAuthClient.auth.signInWithOAuth({ provider: "google", options: { redirectTo: ... } })`
   を呼び、Googleの認可画面へリダイレクトする。
2. **GoTrue**: Google側での認可完了後、`redirectTo`（`/users/new?code=...`）にリダイレクトされる。
   `UserCreatePage`の`useEffect`がURLの`code`クエリを検出し、
   `supabaseAuthClient.auth.exchangeCodeForSession(code)`でPKCEのコード交換を行う。この時点で
   GoTrueが`auth.users`に行をINSERTする（新規ユーザーの場合）、または既存行を参照する
   （既存ユーザーの再ログインの場合）。
3. **Postgres（トリガー）**: `auth.users`へのINSERTをトリガーに`public.profiles`へ行を自動生成する
   （後述§3）。
4. **フロント（フォールバック）**: コード交換が成功したら、`ensureProfile(user.id, { email, displayName })`
   （`PUT /api/users/:id/profile`）を必ず呼ぶ。新規ユーザーならこの時点で`profiles`は既に
   トリガーにより存在しているため何もせず既存行を返し、既存`auth.users`の初回ログインなら
   ここで初めて`profiles`が作られる（後述§4）。

## 3. トリガー本体（新規INSERT時のみ発火）

`worker/drizzle/0001_profile_on_signup_trigger.sql`:

```sql
CREATE OR REPLACE FUNCTION public.handle_new_user()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER SET search_path = ''
AS $$
BEGIN
  INSERT INTO public.profiles (id, email, display_name)
  VALUES (
    NEW.id,
    NEW.email,
    COALESCE(NEW.raw_user_meta_data->>'full_name', NEW.raw_user_meta_data->>'name', NEW.email)
  );
  RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS on_auth_user_created ON auth.users;

CREATE TRIGGER on_auth_user_created
  AFTER INSERT ON auth.users
  FOR EACH ROW EXECUTE PROCEDURE public.handle_new_user();
```

Supabase公式が示す標準パターンで、`display_name`はGoogleの`user_metadata`（`full_name`→
`name`の順）が無ければ`email`で埋める。`SECURITY DEFINER SET search_path = ''`により、
呼び出し元（GoTrue経由のINSERT）のロールに関わらず`public.profiles`へ書き込める。

`worker/src/db/schema.ts`では、`auth.users`はdrizzle-kitがCREATE/ALTER migrationを生成しない
ための最小スタブとして参照専用で定義している。

```ts
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
```

### 制約: 「新規INSERT時のみ」発火する

このトリガーは`AFTER INSERT ON auth.users`なので、**`auth.users`に既に行が存在するユーザーが
再ログイン（またはトークンリフレッシュ等）してもINSERTは発生せず、トリガーは発火しない**。
GoTrueは既存ユーザーのログイン時にはUPDATE（`last_sign_in_at`更新等）を行うだけで、INSERTは
行わない。

### 実際に踏んだ不具合

検証用の別ページ（`frontend/src/pages/oauth-sandbox`、Google連携そのものの動作確認用
サンドボックス）で、Googleアカウント`koma4024@gmail.com`が事前に一度ログイン済みだった
ため、`auth.users`には既にこのユーザーの行が存在していた。この状態で「ユーザー登録」
ページ（`UserCreatePage`）から同じGoogleアカウントでログインすると、`auth.users`側は
UPDATEのみでINSERTが発生せず、トリガーが発火しないため`profiles`行が作られなかった。
結果、ログイン成功後に遷移する詳細画面（`GET /api/users/:id`）が404になった。

この事例は「トリガーだけに依存すると、アプリの外（別の検証ページ等）で先に`auth.users`に
行ができていた既存ユーザーを取りこぼす」という一般的な落とし穴を示している。

## 4. 対処: `ensureProfile`によるフォールバック

トリガー単体では新規INSERT時しか保証できないため、フロントがOAuthコード交換成功後に
**必ず**`ensureProfile`を呼ぶことで、新規サインアップ・既存ユーザーの初回ログインの両方で
`profiles`の存在を保証する構成にした。

`worker/src/routes/users.ts`の`PUT /:id/profile`:

```ts
usersRoute.put("/:id/profile", async (c) => {
  const id = c.req.param("id");
  const body = await c.req.json<{ email?: string; displayName?: string }>();
  if (!body.email || !body.displayName) {
    return c.json({ error: "email, displayName は必須です" }, 400);
  }

  const rows = await withDb(c.env.DATABASE_URL, (db) =>
    db
      .insert(profiles)
      .values({ id, email: body.email!, displayName: body.displayName! })
      .onConflictDoNothing()
      .returning(),
  );
  if (rows[0]) {
    return c.json(rows[0], 201);
  }
  // 既に存在する場合は既存の行をそのまま返す（表示名の上書きはしない）。
  const existing = await withDb(c.env.DATABASE_URL, (db) =>
    db.select().from(profiles).where(eq(profiles.id, id)),
  );
  return c.json(existing[0]);
});
```

`onConflictDoNothing()`（SQLの`INSERT ... ON CONFLICT DO NOTHING`相当）により、既に
`profiles`行が存在する場合（＝トリガーが既に作っていた場合）は何もせず既存行をそのまま
返す。表示名を毎回上書きしない設計のため、ユーザーが後から`PATCH /:id`で表示名を変更していても
再ログインで消えることはない。

フロント側の呼び出し（`frontend/src/entities/user/api/userApi.ts`）:

```ts
// Google OAuthログイン直後に呼ぶ。新規サインアップなら on_auth_user_created
// トリガーが既に profiles を作成済み、既存 auth.users の初回ログインなら
// このリクエストで profiles を作成する（存在すれば何もしない）。
export function ensureProfile(
  id: string,
  input: { email: string; displayName: string },
): Promise<User> {
  return fetch(`/api/users/${id}/profile`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  }).then(parseJsonOrThrow);
}
```

呼び出し元（`frontend/src/pages/user-create/ui/UserCreatePage.tsx`）は、
`exchangeCodeForSession`成功後に`ensureProfile`を呼んでから詳細画面へ`navigate`する。

```tsx
supabaseAuthClient.auth.exchangeCodeForSession(code).then(async ({ data, error }) => {
  // ...
  const { user } = data.session;
  const displayName =
    (user.user_metadata.full_name as string | undefined) ??
    (user.user_metadata.name as string | undefined) ??
    user.email ??
    "";

  try {
    await ensureProfile(user.id, { email: user.email ?? "", displayName });
    navigate(`/users/${user.id}`);
  } catch (ensureError) {
    setStatus("error");
    setErrorMessage((ensureError as Error).message);
  }
});
```

**設計上のポイント**: トリガーとフォールバックはどちらか一方ではなく併用する。トリガーは
「新規サインアップの通常経路を最短で完結させる」役割、`ensureProfile`は「トリガーが発火しない
経路（既存`auth.users`の再ログイン）を救う保険」の役割で、責務が異なる。

## 5. 削除時のCASCADE設計

### 当初の実装バグ

削除エンドポイントの当初実装は「先に`profiles`をDELETEし、その後にAdmin API
（`auth.admin.deleteUser`）で`auth.users`を削除する」という順序だった。この順序では、
`profiles`のDELETEが成功した後にAdmin API呼び出しが失敗すると、**`profiles`だけが消えて
`auth.users`が残る**という不整合が発生するバグがあった（Admin API呼び出しはネットワーク経由の
外部サービス呼び出しであり、DBのDELETEより失敗しやすい）。

### 修正: 外部キーCASCADEに一本化

`worker/src/db/schema.ts`で`profiles.id`は`auth.users.id`への`ON DELETE CASCADE`外部キーとして
定義されている。

```ts
export const profiles = pgTable("profiles", {
  id: uuid("id")
    .primaryKey()
    .references(() => authUsers.id, { onDelete: "cascade" }),
  // ...
});
```

この制約を活かし、`profiles`の明示的なDELETEを削除して「Admin APIで`auth.users`を削除する
だけ」にした。`auth.users`側の削除がPostgresのCASCADEによって`profiles`側の削除を保証するため、
「`profiles`だけ消えて`auth.users`が残る」という不整合は構造的に発生しなくなる（逆に
「`auth.users`だけ消えて`profiles`が残る」ことも無い）。

`worker/src/routes/users.ts`の`DELETE /:id`:

```ts
usersRoute.delete("/:id", async (c) => {
  const id = c.req.param("id");
  const rows = await withDb(c.env.DATABASE_URL, (db) =>
    db.select({ id: profiles.id }).from(profiles).where(eq(profiles.id, id)),
  );
  if (!rows[0]) {
    return c.json({ error: "ユーザーが見つかりません" }, 404);
  }
  // profiles.id は auth.users.id への ON DELETE CASCADE 外部キーのため、
  // auth.users 側を削除すれば profiles も自動的に連動削除される。
  await deleteAuthUser(c.env.SUPABASE_URL, c.env.SUPABASE_SERVICE_ROLE_KEY, id);
  return c.body(null, 204);
});
```

`deleteAuthUser`（`worker/src/services/supabaseAdmin.ts`）は`admin.auth.admin.deleteUser(userId)`
を呼ぶだけの薄いラッパーで、`profiles`には一切触れない。

## 6. GoTrueの`auth.users`への直接SQL INSERTでテストする際の落とし穴

トリガー動作を検証する際、GoTrueのAPI経由ではなく直接SQLで`auth.users`にテスト行を
INSERTして確認する方法を試みたが、GoTrue Admin APIが以降そのユーザーを
**"Database error loading user"** エラーで扱えなくなる既知の問題に遭遇した。

原因は、`auth.users`テーブルの一部カラム（`confirmation_token` / `email_change` /
`email_change_token_new` / `recovery_token`等）がGoTrue内部の実装上**空文字列（`''`）である
ことを要求されており、`NULL`を許容しない**ため。素朴に必要最小限のカラム（`id` / `email`等）
のみ指定してINSERTすると、これらのトークン系カラムが`NULL`のままになり、GoTrue Admin API側の
デコード処理が失敗する。

**実用的な注意点**: 今後同様に「GoTrueを経由せず直接SQLで`auth.users`にテストユーザーを
作る」検証を行う場合は、上記のトークン系カラムに明示的に空文字列を指定すること。あるいは、
直接SQL INSERTに頼らず、実際にGoogle OAuthフロー（またはGoTrue自身のAPI）を経由して
`auth.users`行を作る方が、GoTrue自身が要求する内部整合性を確実に満たせるため安全である。
本タスクでの検証は最終的に実際のGoogleログインフローで行い、この問題を回避した。

## 7. 関連ドキュメント

- `doc_arch/backend.md` §3（自前API層でSupabaseを隠蔽するポータビリティ方針。ロックイン表に
  「認証」行あり）
- `doc_arch/frontend.md` §3（認証フローに限りフロントがSupabase Auth JS SDKを直接使ってよい
  例外規定）
- `docs_bevy_sample/20260816_local-gotrue-google-login-troubleshooting.md`（ローカルGoTrue自体の
  セットアップ・CORSトラブルシューティング）
- `docs_bevy_sample/20260816_google-oauth-client-setup-for-local-gotrue.md`（Google Cloud
  Console側の手動セットアップ手順）
- `worker/drizzle/0001_profile_on_signup_trigger.sql`（トリガー本体）
- `worker/src/db/schema.ts`（`authUsers`スタブ・`profiles`テーブル定義）
- `worker/src/routes/users.ts`（`PUT /:id/profile` ensureProfile相当・`DELETE /:id`）
- `worker/src/services/supabaseAdmin.ts`（`deleteAuthUser`）
- `frontend/src/entities/user/api/supabaseAuthClient.ts`（Supabase Authクライアント、PKCE flow）
- `frontend/src/entities/user/api/userApi.ts`（`ensureProfile`）
- `frontend/src/pages/user-create/ui/UserCreatePage.tsx`（`signInWithOAuth`→
  `exchangeCodeForSession`→`ensureProfile`→詳細画面へ`navigate`）
