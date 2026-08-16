// 動作確認用の最小Edge Function。
//
// 公式サンプルは @supabase/server の withSupabase({ auth: ["publishable", "secret"] })
// を使い、新形式APIキー(sb_publishable_*/sb_secret_*)での検証を要求するが、
// このプロジェクトはレガシーAPIキー(ANON_KEY/SERVICE_ROLE_KEY)のみを使う方針
// (docs_bevy_sample/20260816_api-gateway-migration-implementation-options.md 参照)
// のため、その検証方式とは噛み合わない。ここでは認証チェックを行わず、
// functionsサービス自体が正しく動作するかの確認に用途を絞る。

console.log("Hello from Functions!");

Deno.serve(() => Response.json({ message: "Hello from Edge Functions!" }));
