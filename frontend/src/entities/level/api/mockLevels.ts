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
    // LevelCard のサムネイル（object-fit: cover）で縦長画像がどう見えるかのサンプル。
    // tall_portrait_sample.png は 500x1500（1:3）の極端な縦長画像で、上端に黄色地+赤丸+"TOP"、
    // 下端に青地+"BOTTOM"のマーカーを置いてある。Ameca_robot.jpg（962x1080）は縦長といっても
    // ほぼ正方形に近く、中央クロップとの違いが分かりにくかったため、より縦横比の大きい画像で
    // 検証できるように追加した。
    // - LevelCard.css の object-position: top により、レベル一覧のサムネイルでは
    //   上端の "TOP" マーカーが残り、下端の "BOTTOM" は見切れる（中央寄せなら逆に、
    //   どちらのマーカーも見えず中央のグラデーションだけが見えるはず）。実際に Playwright で
    //   確認済み。
    // - ゲーム内背景は contain_fit なのでクロップ自体は起きないが、bricks を空にしたこの
    //   レベルは brickImage 未指定のデフォルト単色ブロックがアリーナ上部のほぼ全域を覆うため、
    //   実際のプレイ画面では画像の上端（"TOP" マーカー）はブロックの下に隠れて見えない
    //   （見えるのはブロック帯の下、パドルの少し上に覗く下端付近の細い帯だけ）。
    //   「クロップされない」ことと「他の不透明な要素に隠れない」ことは別問題という一例。
    id: "tall-portrait-sample",
    title: "縦長画像サンプル（サムネイル上寄せ確認用）",
    author: "system",
    thumbnailUrl: "/assets/backgrounds/tall_portrait_sample.png",
    background: "/assets/backgrounds/tall_portrait_sample.png",
    cellSize: { width: 50, height: 30 },
    bricks: [],
  },
  {
    // ゲーム内背景（contain_fit、game_engine/src/util.rs::inscribed_source_rect）の
    // 上寄せ確認用サンプル。wide_landscape_sample.png は 1800x400（4.5:1）の極端な横長画像。
    // アリーナは 900x600（1.5:1）なので、この画像は幅基準で内接し、上下に余白ができる
    // （縦長画像は高さ基準で内接するため上下の余白が生まれず、上寄せ/中央寄せの差が出ない。
    // 差が出るのはこの画像のようにアリーナよりアスペクト比が大きい横長画像の場合だけ）。
    // contain は画像を一切クロップしない（LevelCard.css の object-fit: cover とは違い、
    // 常に画像全体を縮小して収める）ため、上寄せにしても上端・下端どちらのマーカーも
    // 見えなくなりはしない。実際に起きるのは「画像全体がアリーナ天井まで押し上げられ、
    // 余白（レターボックス）が下側だけにまとまる」こと（中央寄せなら上下に半分ずつ余白が
    // 出ていたはず）。上端の緑帯+"TOP"がアリーナ天井にぴったり揃うことと、下端の紫帯+"BOTTOM"
    // より下がすべて余白（黒）になっていることの両方を Playwright で確認済み。
    // bricks を空にするとデフォルトの単色ブロックがアリーナ上部を覆って上側が隠れてしまう
    // （tall-portrait-sample で判明した問題）ため、brickImage にも同じ画像を指定し、
    // ブロック越しでも同じ絵がシームレスに見えるようにしている。
    id: "wide-landscape-top-align-sample",
    title: "横長画像サンプル（ゲーム内背景の上寄せ確認用）",
    author: "system",
    thumbnailUrl: "/assets/backgrounds/wide_landscape_sample.png",
    background: "/assets/backgrounds/wide_landscape_sample.png",
    brickImage: "/assets/backgrounds/wide_landscape_sample.png",
    cellSize: { width: 50, height: 30 },
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
