import { defineConfig, type PluginOption } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";

// Bevy の WASM / アセット配信を dev サーバーで正しく扱うためのプラグイン。
function bevyDevServer(): PluginOption {
  const watchDir = resolve(import.meta.dirname, "public/assets/backgrounds");
  return {
    name: "bevy-dev-server",
    configureServer(server) {
      // (1) `*.meta` へのリクエストは 404 を返す。
      // Bevy は各アセット取得時に `<asset>.meta` も fetch するが、存在しないと
      // Vite の SPA フォールバックが index.html(200) を返してしまい、Bevy が
      // それを RON メタとして解析失敗 → アセットのロードが中断される。
      // 404 を返せば Bevy は「メタ無し（既定）」として本体を正しく読み込む。
      server.middlewares.use((req, res, next) => {
        const path = (req.url ?? "").split("?")[0];
        if (path.endsWith(".meta")) {
          res.statusCode = 404;
          res.end();
          return;
        }
        next();
      });

      // (2) public/assets/backgrounds/ の画像を差し替えたら自動フルリロード。
      // public 配下はモジュールグラフに乗らず HMR が効かないため watcher を明示する。
      server.watcher.add(watchDir);
      const isImage = (f: string) => /\.(png|jpe?g|webp|gif|avif)$/i.test(f);
      const trigger = (file: string) => {
        if (file.startsWith(watchDir) && isImage(file)) {
          server.ws.send({ type: "full-reload", path: "*" });
          server.config.logger.info(
            `[background] changed: ${file.replace(watchDir, "backgrounds")} → full reload`,
          );
        }
      };
      server.watcher.on("add", trigger);
      server.watcher.on("change", trigger);
      server.watcher.on("unlink", trigger);
    },
  };
}

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), bevyDevServer()],
  server: {
    // Bevy(wgpu/WebGL2) と wasm-bindgen 出力を安定動作させるための設定。
    fs: { strict: false },
  },
});
