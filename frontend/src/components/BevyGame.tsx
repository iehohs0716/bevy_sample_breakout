import { useEffect, useRef } from "react";

type BevyGameProps = {
  width?: number;
  height?: number;
  /**
   * 背景画像の URL。同一オリジンの相対パス（例: `/assets/backgrounds/sample_sunset.png`）でも、
   * 外部の絶対 URL（例: `https://cdn.example.com/bg/xxx.png`）でも指定できる。
   *
   * ここで渡した画像を React 側が fetch し、バイト列を WASM(Bevy) に引き渡す。これにより
   * ゲーム本体（Rust/WASM）は 1 つのビルドのまま、サービスごとに背景だけを差し替えられる。
   * 外部 URL を使う場合、配信元が CORS を許可している必要がある（ブラウザの fetch を通すため）。
   * 未指定の場合は Bevy 側にバンドルされたデフォルト背景が使われる。
   */
  background?: string;
};

/**
 * WASM 化した Bevy Breakout を canvas に埋め込むコンポーネント。
 *
 * wasm-bindgen(`--target web`) が public/wasm/ に出力する JS グルー(`breakout.js`)を
 * 実行時に動的 import し、default export の init() を呼ぶと Bevy が起動して
 * `#bevy-canvas` に描画する。
 *
 * `background` が指定されていれば、init() の前にその画像を fetch して
 * `window.__BREAKOUT_CONFIG__.backgroundBytes`(Uint8Array) に載せる。Bevy 側(Rust)は
 * 起動時にこれを読み、`Image::from_buffer` でデコードして背景スプライトに使う。
 */
export function BevyGame({ width = 900, height = 600, background }: BevyGameProps) {
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
      // 背景画像を React 側で先に fetch し、バイト列を window のグローバル設定に載せる。
      // Bevy(WASM) の起動時(main)にこれを読むので、必ず init() より前に行う。
      // fetch に失敗した場合は Bevy 側のデフォルト背景にフォールバックさせる（致命ではない）。
      if (background) {
        try {
          const res = await fetch(background);
          if (!res.ok) {
            throw new Error(`背景画像の取得に失敗: ${res.status} ${res.statusText}`);
          }
          const buf = await res.arrayBuffer();
          const w = window as typeof window & {
            __BREAKOUT_CONFIG__?: {
              backgroundBytes?: Uint8Array;
              backgroundMime?: string;
            };
          };
          w.__BREAKOUT_CONFIG__ = {
            backgroundBytes: new Uint8Array(buf),
            // フォーマット判定用の MIME（例: "image/png"）。取得できなければ Bevy 側は png とみなす。
            backgroundMime: res.headers.get("content-type") ?? undefined,
          };
        } catch (error) {
          console.warn(
            "背景画像の取得に失敗しました。デフォルト背景で起動します。",
            error,
          );
        }
      }

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
    // 依存配列は空にする。背景は起動時に一度だけ読む値であり、Bevy(winit) は
    // 再初期化・破棄ができないため、background が後から変わっても再起動しない
    // （初回マウント時の値で固定される）。effect 内で background を参照しているので
    // exhaustive-deps は警告するが、上記の理由で意図的に無視する。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return <canvas id="bevy-canvas" width={width} height={height} />;
}
