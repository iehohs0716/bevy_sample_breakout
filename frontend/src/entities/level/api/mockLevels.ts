// レベル一覧画面・プレイ画面が参照するモックデータ。
// 実データ取得（API等）が実装されるまでの暫定データとして、レベルの静的な配列を提供する。

import type { Level } from "../model/types";
import { buildPyramidLayout, buildCenteredBlockLayout } from "../lib/buildLayout";

export const MOCK_LEVELS: Level[] = [
  {
    id: "robot-pyramid",
    title: "ロボットの丘",
    author: "system",
    thumbnailUrl: "/assets/backgrounds/Ameca_robot.jpg",
    background: "/assets/backgrounds/Ameca_robot.jpg",
    brickImage: "/assets/backgrounds/sample_grid.png",
    cellSize: { width: 30, height: 30 },
    bricks: buildPyramidLayout({ width: 30, height: 30 }),
  },
  {
    id: "sunset-default",
    title: "夕焼けの壁",
    author: "system",
    thumbnailUrl: "/assets/backgrounds/sample_sunset.png",
    background: "/assets/backgrounds/sample_sunset.png",
    cellSize: { width: 50, height: 30 },
    bricks: [],
  },
  {
    id: "grid-block",
    title: "格子の砦",
    author: "system",
    thumbnailUrl: "/assets/backgrounds/background.png",
    background: "/assets/backgrounds/background.png",
    brickImage: "/assets/backgrounds/sample_grid.png",
    cellSize: { width: 50, height: 30 },
    bricks: buildCenteredBlockLayout({ width: 50, height: 30 }),
  },
  {
    // 外部ホストの画像をbackgroundに指定する例。picsum.photosはCORSを許可しているため、
    // 同一オリジンの/assets配下の画像と同様にReact側からfetchしてBevyへ渡せる
    // （docs_bevy_sample/20260711_external-image-cors-and-formats.md参照。配信元が
    // CORSを許可していない外部URLは、この方法では読み込めない点に注意）。
    id: "external-cors-sample",
    title: "旅の記憶",
    author: "system",
    thumbnailUrl: "https://picsum.photos/id/1015/900/600.jpg",
    background: "https://picsum.photos/id/1015/900/600.jpg",
    cellSize: { width: 30, height: 30 },
    bricks: buildPyramidLayout({ width: 30, height: 30 }),
  },
];
