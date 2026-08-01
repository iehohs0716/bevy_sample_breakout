import type { BevyGameProps } from "../model/types";
import { fetchImageBytes } from "./fetchImageBytes";

type StartBevyGameOptions = Pick<
  BevyGameProps,
  "background" | "bricks" | "cellSize" | "brickImage"
>;

type BreakoutConfig = {
  backgroundBytes?: Uint8Array;
  backgroundMime?: string;
  bricks?: Array<{ x: number; y: number }>;
  cellSize?: { width: number; height: number };
  brickImage?: { bytes: Uint8Array; mime?: string };
};

/**
 * Bevy(WASM) を起動する。React 側の初期化パラメータ（背景・初期ブロック配置）を
 * window.__BREAKOUT_CONFIG__ にまとめて載せてから、wasm-bindgen の JS グルーを
 * ロードして init() を呼ぶ。Bevy(winit) は同一ページ内での再初期化に対応していないため、
 * 呼び出し元（BevyGame）でマウント中に一度だけ呼ばれることを前提とする。
 */
export async function startBevyGame({
  background,
  bricks,
  cellSize,
  brickImage,
}: StartBevyGameOptions): Promise<void> {
  const config: BreakoutConfig = {};

  // 背景画像は React 側で先に fetch し、バイト列を渡す。
  if (background) {
    const image = await fetchImageBytes(background, "背景画像");
    if (image) {
      config.backgroundBytes = image.bytes;
      config.backgroundMime = image.mime;
    }
  }

  // 初期ブロック配置。指定があれば渡す。無ければ Bevy 側のデフォルト配置になる。
  if (bricks && bricks.length > 0) {
    config.bricks = bricks;
    if (cellSize) {
      config.cellSize = cellSize;
    }
  }

  // ブロック用の画像。背景と同様に React 側で fetch してバイト列を渡す。
  if (brickImage) {
    config.brickImage = await fetchImageBytes(brickImage, "ブロック画像");
  }

  const w = window as typeof window & {
    __BREAKOUT_CONFIG__?: BreakoutConfig;
  };
  w.__BREAKOUT_CONFIG__ = config;

  // public 配下の生成物なので Vite のモジュール解決を通さず、実行時に完全な
  // 絶対 URL を組み立てて外部モジュールとして import する（@vite-ignore で警告抑制）。
  // これにより Vite の「/public を import 不可」ガードを回避する。dev/本番とも
  // 同じ `/wasm/breakout.js` パスで動作する。
  const wasmUrl = new URL("/wasm/breakout.js", window.location.origin).href;
  const wasmModule = await import(/* @vite-ignore */ wasmUrl);

  // クエリでキャッシュを無効化しない。約25MBあり、レベル切り替えのたびのフルリロード
  // (BevyはWASM初期化をページ内でやり直せないため、レベル切り替えは毎回フルリロードになる)
  // で毎回再ダウンロードされると重すぎるため、ブラウザキャッシュに乗せる。ビルドし直した後に
  // 古いWASMが残る場合は、開発者側でハードリロード（キャッシュ無視の再読み込み）すればよい。
  const wasmBin = new URL("/wasm/breakout_bg.wasm", window.location.origin)
    .href;

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
}
