import { useEffect, useRef } from "react";
import type { BevyGameProps } from "../model/types";
import { useBreakoutGameEvents } from "../lib/useBreakoutGameEvents";
import { startBevyGame } from "../lib/startBevyGame";

/**
 * WASM 化した Bevy Breakout を canvas に埋め込むコンポーネント。
 *
 * 実際の起動処理（画像 fetch・WASM ロード・init() 呼び出し）は `lib/startBevyGame` に、
 * Bevy(WASM) → フロントのイベント購読は `lib/useBreakoutGameEvents` に委譲し、
 * ここではマウント時に一度だけ起動する制御と canvas の描画だけを担う。
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

  useBreakoutGameEvents(onGameClear, onGameOver);

  useEffect(() => {
    if (startedRef.current) return;
    startedRef.current = true;

    void startBevyGame({ background, bricks, cellSize, brickImage });
  }, []);

  return <canvas id="bevy-canvas" width={width} height={height} />;
}
