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
  /**
   * 初期ブロック配置。各ブロックの中心座標を Bevy のワールド座標で指定する。
   * 座標系は「中心原点・y 上向き・1 単位 = 1px」で、アリーナは x∈[-450, 450],
   * y∈[-300, 300]（画面中央が原点、上が +y）。背景と同様に起動時に一度だけ
   * `window.__BREAKOUT_CONFIG__` 経由で WASM(Bevy) に渡す。
   * 未指定 / 空配列なら Bevy 側のデフォルト配置（アリーナ敷き詰め）にフォールバックする。
   */
  bricks?: Array<{ x: number; y: number }>;
  /**
   * `bricks` で配置したブロック共通のセルの大きさ（幅・高さ、px 相当）。
   * `bricks` とセットで指定したときのみ効く（`cellSize` 単独では無視される。
   * `bricks` 未指定時のデフォルト敷き詰め配置は Bevy 側の固定サイズ 50x30 を使うため）。
   * `bricks` を渡しつつ `cellSize` を省いた場合は Bevy 側のデフォルトサイズ（50x30）になる。
   */
  cellSize?: { width: number; height: number };
  /**
   * ブロックの見た目に使う画像の URL。背景と同様に React 側が fetch し、バイト列を WASM(Bevy)
   * に渡す。盤面全体にこの画像を比率維持で貼ったとみなし、各ブロックはその絵のうち自分が覆う
   * 領域だけを切り出して表示する（全ブロックが揃うと 1 枚の絵になり、壊すと背景画像が覗く）。
   * 同一オリジンの相対パスでも外部の絶対 URL でも指定可能（外部 URL は配信元の CORS 許可が必要）。
   * 未指定 / fetch 失敗なら Bevy 側の単色ブロックにフォールバックする。
   */
  brickImage?: string;
  /**
   * ゲームクリア（全ブロック破壊）を Bevy(WASM) が検知したときに呼ばれる。
   * Bevy 側は `window.dispatchEvent(new CustomEvent("breakout:gameclear",
   * { detail: { result: "clear", score } }))` を投げるだけで、遷移（リロード/画面移動）は
   * React が担う。未指定の場合は既定で `window.location.reload()`（＝リロードして次ゲーム）を
   * 行う。遷移先を変えたい場合はこのコールバックで上書きする（例: 結果画面へ `location.href = ...`）。
   */
  onGameClear?: (detail: { result: string; score: number }) => void;
  /**
   * ゲームオーバー（ライフ 0）を Bevy(WASM) が検知したときに呼ばれる（`breakout:gameover`、
   * `detail.result === "gameover"`）。`onGameClear` と対称で、未指定の場合は既定で
   * `window.location.reload()`（＝リロードして最初から）を行う。ゲームオーバー専用の遷移に
   * したい場合はこのコールバックで上書きする。クリアとの区別は `detail.result` で行う。
   */
  onGameOver?: (detail: { result: string; score: number }) => void;
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
export function BevyGame({
  width = 900,
  height = 600,
  background,
  bricks,
  cellSize,
  brickImage,
  onGameClear,
  onGameOver,
}: BevyGameProps) {
  // React StrictMode は開発時に effect を2回実行する。Bevy(winit) は二重初期化で
  // パニックするため、ref ガードで一度だけ起動する。
  const startedRef = useRef(false);

  // Bevy(WASM) → フロントのゲームイベントを受ける。遷移（リロード等）は React が担う。
  // Bevy 側は状態遷移時に CustomEvent を投げるだけで、URL は一切知らない。
  useEffect(() => {
    const handleGameClear = (e: Event) => {
      const detail = (e as CustomEvent<{ result: string; score: number }>).detail;
      if (onGameClear) {
        onGameClear(detail);
      } else {
        // 既定挙動: リロードして次ゲーム。次ゲームのパラメータを差し替えたい場合は
        // onGameClear で上書きし、window.__BREAKOUT_CONFIG__ を書き換えてから reload する。
        window.location.reload();
      }
    };
    const handleGameOver = (e: Event) => {
      const detail = (e as CustomEvent<{ result: string; score: number }>).detail;
      if (onGameOver) {
        onGameOver(detail);
      } else {
        // 既定挙動: クリアと対称に、リロードして最初から遊べるようにする。
        // ゲームオーバー専用の遷移（結果画面へ移動など）にしたい場合は onGameOver で上書きする。
        // クリアかゲームオーバーかは detail.result（"clear" / "gameover"）で区別できる。
        window.location.reload();
      }
    };

    window.addEventListener("breakout:gameclear", handleGameClear);
    window.addEventListener("breakout:gameover", handleGameOver);
    return () => {
      window.removeEventListener("breakout:gameclear", handleGameClear);
      window.removeEventListener("breakout:gameover", handleGameOver);
    };
  }, [onGameClear, onGameOver]);

  useEffect(() => {
    if (startedRef.current) return;
    startedRef.current = true;

    (async () => {
      // React 側の初期化パラメータ（背景・初期ブロック配置）を window のグローバル設定に
      // まとめて載せる。Bevy(WASM) は起動時(main)に一度だけこれを読むので、必ず init() より
      // 前に用意する。個々の値が無ければ Bevy 側がそれぞれデフォルトにフォールバックする。
      const config: {
        backgroundBytes?: Uint8Array;
        backgroundMime?: string;
        bricks?: Array<{ x: number; y: number }>;
        cellSize?: { width: number; height: number };
        brickImage?: { bytes: Uint8Array; mime?: string };
      } = {};

      // 背景画像は React 側で先に fetch し、バイト列を渡す。
      // fetch に失敗した場合は Bevy 側のデフォルト背景にフォールバックさせる（致命ではない）。
      if (background) {
        try {
          const res = await fetch(background);
          if (!res.ok) {
            throw new Error(`背景画像の取得に失敗: ${res.status} ${res.statusText}`);
          }
          const buf = await res.arrayBuffer();
          config.backgroundBytes = new Uint8Array(buf);
          // フォーマット判定用の MIME（例: "image/png"）。取得できなければ Bevy 側は png とみなす。
          config.backgroundMime = res.headers.get("content-type") ?? undefined;
        } catch (error) {
          console.warn(
            "背景画像の取得に失敗しました。デフォルト背景で起動します。",
            error,
          );
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
      // fetch に失敗した場合は Bevy 側の単色ブロックにフォールバックさせる（致命ではない）。
      if (brickImage) {
        try {
          const res = await fetch(brickImage);
          if (!res.ok) {
            throw new Error(`ブロック画像の取得に失敗: ${res.status} ${res.statusText}`);
          }
          const buf = await res.arrayBuffer();
          config.brickImage = {
            bytes: new Uint8Array(buf),
            mime: res.headers.get("content-type") ?? undefined,
          };
        } catch (error) {
          console.warn(
            "ブロック画像の取得に失敗しました。単色ブロックで起動します。",
            error,
          );
        }
      }

      const w = window as typeof window & {
        __BREAKOUT_CONFIG__?: typeof config;
      };
      w.__BREAKOUT_CONFIG__ = config;

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
