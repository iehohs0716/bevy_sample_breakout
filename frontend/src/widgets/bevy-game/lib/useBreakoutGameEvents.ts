import { useEffect } from "react";
import type { BevyGameProps } from "../model/types";

type GameEventHandler = NonNullable<BevyGameProps["onGameClear"]>;

/**
 * Bevy(WASM) → フロントのゲームイベントを受ける。遷移（リロード等）は React が担う。
 * Bevy 側は状態遷移時に CustomEvent を投げるだけで、URL は一切知らない。
 */
export function useBreakoutGameEvents(
  onGameClear?: GameEventHandler,
  onGameOver?: GameEventHandler,
): void {
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
}
