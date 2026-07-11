import { useEffect, useRef } from "react";

type BevyGameProps = {
  width?: number;
  height?: number;
};

/**
 * WASM 化した Bevy Breakout を canvas に埋め込むコンポーネント。
 *
 * wasm-bindgen(`--target web`) が public/wasm/ に出力する JS グルー(`breakout.js`)を
 * 実行時に動的 import し、default export の init() を呼ぶと Bevy が起動して
 * `#bevy-canvas` に描画する。
 */
export function BevyGame({ width = 900, height = 600 }: BevyGameProps) {
  // React StrictMode は開発時に effect を2回実行する。Bevy(winit) は二重初期化で
  // パニックするため、ref ガードで一度だけ起動する。
  const startedRef = useRef(false);

  useEffect(() => {
    // ref ガードで初期化を一度だけに絞る。ref はマウントをまたいで保持されるため、
    // StrictMode の二重 effect（マウント→クリーンアップ→再マウント）でも 2 回目は起動しない。
    // Bevy(winit) は再初期化・破棄ができないので、クリーンアップで init を中断しない
    // （中断すると唯一の初期化が止まって何も表示されなくなる）。
    if (startedRef.current) return;
    startedRef.current = true;

    (async () => {
      // public 配下の生成物なので Vite のモジュール解決を通さず、実行時に完全な
      // 絶対 URL を組み立てて外部モジュールとして import する（@vite-ignore で警告抑制）。
      // これにより Vite の「/public を import 不可」ガードを回避する。dev/本番とも
      // 同じ `/wasm/breakout.js` パスで動作する。
      const wasmUrl = new URL("/wasm/breakout.js", window.location.origin).href;
      const wasmModule = await import(/* @vite-ignore */ wasmUrl);

      // .wasm(約57MB)はファイル名が固定のためブラウザが旧ビルドをキャッシュしやすい。
      // クエリを付けて明示的に渡し、リロード時に必ず最新を読ませる（開発時のキャッシュ事故防止）。
      const wasmBin = new URL(
        `/wasm/breakout_bg.wasm?t=${Date.now()}`,
        window.location.origin,
      ).href;

      const init = wasmModule.default as (options?: {
        module_or_path?: string;
      }) => Promise<unknown>;
      await init({ module_or_path: wasmBin }).catch((error: Error) => {
        // winit は制御フローに例外を使うため、この特定メッセージは無視する。
        if (
          !error.message?.startsWith(
            "Using exceptions for control flow, don't mind me. This isn't actually an error!",
          )
        ) {
          throw error;
        }
      });
    })();
  }, []);

  return <canvas id="bevy-canvas" width={width} height={height} />;
}
