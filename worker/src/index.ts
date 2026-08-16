import { Hono } from "hono";
import type { Env } from "./env";
import { usersRoute } from "./routes/users";

const app = new Hono<{ Bindings: Env }>();

app.route("/api/users", usersRoute);

export default app;
