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
    // ブロック位置を明示指定せず、背景画像とブロック画像の「差分」から自動でブロックを
    // 配置する例（game_engine/src/injection.rs の diff_brick_layout）。bricks を空にすると
    // 明示配置が無いと判定され、背景・ブロック画像が両方指定されているため自動生成が発火する。
    // diff_brick_image.png は diff_background.png に対し、上端の帯（アリーナ天井付近）と
    // 中段の帯（マゼンタ）の 2 箇所だけ絵柄を変えてある。上端の帯は差分ありとしてブロック化
    // されるが、中段の帯は config.rs の BRICK_DIFF_LAYOUT_MIN_HEIGHT_RATIO（高さ制限）次第で
    // 対象外になる（＝差分があってもブロック化されず背景のまま見える）想定。
    // cellSize は bricks が空でも効く（Bevy 側のデフォルト 50x30 ではなく、ここで
    // 指定した粒度で差分判定・ブロック生成が行われることの確認を兼ねる）。
    id: "diff-auto-layout-sample",
    title: "自動配置サンプル（画像差分）",
    author: "system",
    thumbnailUrl: "/assets/backgrounds/diff_brick_image.png",
    background: "/assets/backgrounds/diff_background.png",
    brickImage: "/assets/backgrounds/diff_brick_image.png",
    cellSize: { width: 25, height: 20 },
    bricks: [],
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
