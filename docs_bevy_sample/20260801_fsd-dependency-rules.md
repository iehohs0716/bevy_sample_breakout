# FSDの依存関係のルール

日付: 2026-08-01

## 1. 位置づけ

`docs_bevy_sample/20260801_fsd-overview.md`で整理したFSD（Feature-Sliced Design）の
レイヤー構造について、レイヤー間・スライス間の依存の向きに関するルールを解説する。

関連ドキュメント: `docs_bevy_sample/20260801_fsd-overview.md`（レイヤー・スライス・
セグメントの全体像）、`docs_bevy_sample/20260801_fsd-vs-atomic-design.md`（Atomic
Designとの比較）。

## 2. 基本ルール

依存の方向は次の一方通行。

```
app → pages → widgets → features → entities → shared
```

各レイヤーは自分自身か、自分より下のレイヤーだけをimportしてよい。下のレイヤーは上の
レイヤーの存在を一切知ってはいけない。

## 3. 本リポジトリでの具体例

`entities/level`（特に`entities/level/ui/LevelCard.tsx`）は、自分がどのページ
（`pages/level-list`）で使われているかを一切知らない・依存しない。`LevelCard`は単に
`level`というpropsを受け取ってカードを描画するだけで、「一覧画面専用だ」という情報は
コードのどこにも現れない。これにより、将来別の画面（例えばトップページのおすすめ表示等）
でも同じ`LevelCard`をそのまま使い回せる。

## 4. 実務での強制方法

この依存方向のルールは単なる命名規約ではなく、`steiger`や`eslint-plugin-boundaries`
のようなlintツールで機械的に強制できる。

## 5. 同じレイヤー内のスライス同士の依存

同じレイヤー内のスライス同士も直接importすべきではない（例: `entities/level`が
`entities/user`を直接importするのは避ける）。スライスをまたぐ連携が必要な場合は、
共通の下位レイヤー（`shared`）を介するか、より上位のレイヤー（`widgets`/`pages`）側で
組み合わせる。

## 6. 公開API（Public API）パターン

各スライスは、自分の中身を`index.ts`（バレルファイル）経由でのみ外部に公開する。他の
スライス・レイヤーはこの`index.ts`を通してのみimportし、スライス内部の個別ファイルへ
直接アクセスしてはいけない。

本リポジトリの例: `entities/level/index.ts`が`Level`型・`MOCK_LEVELS`・`LevelCard`を
exportしており、`pages/level-list/ui/LevelListPage.tsx`側は

```ts
import { MOCK_LEVELS, LevelCard } from "@/entities/level";
```

のように、公開APIの`@/entities/level`からimportしている（`@/entities/level/ui/LevelCard`
のような内部パスへ直接importしていない）。この「公開APIを通してしか使わせない」という
カプセル化が、依存方向のルールを実効性のあるものにしている。
