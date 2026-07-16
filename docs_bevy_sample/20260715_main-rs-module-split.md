# main.rs のモジュール分割（config / components / injection / rendering / setup / systems）

日付: 2026-07-15

背景・ブロックの画像機能を足して肥大化した `game_engine/src/main.rs`（約 800 行）を、
責務ごとのモジュールへ分割した記録。**挙動は一切変えない純粋なリファクタ**。

この分割の対象になった機能そのものは [[20260715_brick-image-rendering]] /
[[20260715_aspect-ratio-and-letterbox]]、注入の仕組みは [[20260711_react-to-bevy-init-params]] を参照。

---

## 1. 分割後の構成と責務

すべて `game_engine/src/` 直下のファイル（バイナリクレートのモジュール）。

| ファイル | 責務 | 主な中身 |
|---|---|---|
| `main.rs` | エントリポイント。モジュール宣言と `App` 構築のみ | `mod ...;` / `fn main()` |
| `config.rs` | ゲーム全体の定数 | アリーナ寸法・各サイズ・色・アセットパス |
| `components.rs` | Component / Resource / Event の定義 | `Paddle` `Ball` `Velocity` `Brick` `Wall`/`WallLocation` `Score` `ScoreboardUi` `CollisionSound` `Collider` `BallCollided` |
| `injection.rs` | React(JS) から渡る初期化パラメータの読み取りと保持 | `*Override` リソース群 + `injected_*()` 関数（wasm/native の両 cfg）+ `BrickLayout` |
| `rendering.rs` | 画像フィット計算とブロック描画ヘルパー | `contain_fit` `brick_image_rect` `spawn_brick` |
| `setup.rs` | 起動時セットアップ system | `setup()` |
| `systems.rs` | 毎フレームの system と衝突判定 | `move_paddle` `apply_velocity` `update_scoreboard` `check_for_collisions` `play_collision_sound` / `ball_collision` / `Collision` |

## 2. 依存の向き

一方向で循環なし:

```
config  ← components ← rendering ← setup
   ↑          ↑           ↑         ↑
   └──────────┴───────────┴─────────┴──  injection / systems も config・components に依存
main.rs → injection, setup, systems, components を use して App を組む
```

- `config` は誰にも依存しない（純粋な定数）。
- `components` は `config`（壁の座標・色）に依存。
- `rendering` は `config` + `components`（`Brick`/`Collider` を spawn）に依存。
- `setup` はほぼ全部を use する統合層。
- `systems` は `config` + `components`。

## 3. 可視性の方針

- モジュール跨ぎで使う型・関数・定数は `pub`。
- **タプル構造体のフィールドも `pub` が要る**。`setup` が `background_override.0.take()` の
  ように `.0` へアクセスするため、`pub struct BackgroundOverride(pub Option<Image>);` と
  フィールドまで `pub` にする。`BrickLayout` の `positions` / `cell_size` も同様。
- モジュール内に閉じるものは非公開のまま。例: `systems.rs` の `Collision` enum と
  `ball_collision()` は同ファイル内でしか使わないので `pub` にしない。

## 4. `#[cfg]` 分岐の置き場所（injection.rs）

React 注入は Web 専用なので、各 `injected_*` は wasm と native で 2 実装を持つ:

```rust
#[cfg(target_arch = "wasm32")]
pub fn injected_brick_image() -> Option<Image> { /* web_sys/js_sys で window を読む */ }

#[cfg(not(target_arch = "wasm32"))]
pub fn injected_brick_image() -> Option<Image> { None } // ネイティブは常にデフォルト
```

- native 側でしか使わない / wasm 側でしか使わない `use` は、そのぶん未使用 import 警告が出る。
  今回は `BRICK_SIZE` の import を `#[cfg(target_arch = "wasm32")]` 付きにして回避している。
- `injection.rs` に注入まわりを集約したことで、`web_sys` / `js_sys` / `wasm_bindgen` への依存が
  1 ファイルに閉じ、他モジュールは純粋な Bevy コードのままになった。

## 5. 検証

- `cargo check`（native）と `cargo check --target wasm32-unknown-unknown` の **両方**を通す。
  cfg 分岐があるので片方だけでは不十分。
- `cargo check` は cSpell の警告（`srgb` / `Aabb` / `despawn` など）を出すが無害。
- リファクタ後、両ターゲットとも **警告ゼロ**でコンパイルできることを確認。
- フロントの型（`tsc -b`）も別途通しておく。

## 6. 補足：ビルドと反映

ソース構成を変えただけでは配信物は変わらない。ブラウザ反映には WASM 再ビルドが要る:

```bash
cd frontend && pnpm build:wasm   # cargo build(release, wasm32) → wasm-bindgen → public/wasm/ へ配置
```

WASM 化・ビルドの全体像は [[20260711_bevy-wasm-react-integration]]、ビルドサイズは
[[20260711_wasm-build-size-optimization]]、実ブラウザでの確認は
[[20260711_wasm-bevy-browser-verification]] を参照。
