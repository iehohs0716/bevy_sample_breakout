# Bevy(WASM) のビルドサイズ最適化（cargo エコシステム内で 57MB→25MB）

日付: 2026-07-11

WASM 化した Bevy Breakout の配信サイズを、外部ツール(binaryen/wasm-opt)に頼らず
**cargo エコシステム内**（`Cargo.toml` の feature とプロファイルのみ）で削減した記録。
WASM 化の全体構成は [[20260711_bevy-wasm-react-integration]]、実ブラウザでの描画確認手法は
[[20260711_wasm-bevy-browser-verification]]、React からの初期化パラメータ注入は
[[20260711_react-to-bevy-init-params]] を参照。

---

## 1. 背景 / 課題

- WASM 生成物 `frontend/public/wasm/breakout_bg.wasm` が肥大化していた。
  最適化前で **60,135,378 bytes（約 57.3 MiB）**。初回ロード時間と配信帯域の問題。
- ユーザー要件: 削減は **cargo エコシステム内で完結**させる。
  binaryen/wasm-opt のような外部ツールは今回対象外（不要と明言）。
  `frontend/scripts/build-wasm.sh` の `wasm-opt` ステップは未インストールで
  スキップされる（`==> wasm-opt が無いためサイズ最適化はスキップ` ログ）が、
  今回はそれに依存せず素の cargo 成果物で削る方針。

## 2. 調査（Bevy 公式ドキュメント）

参照: bevy-cheatbook `platforms/wasm/size-opt.html`、および docs.rs/bevy の
Cargo Features / Profiles。

Bevy 0.19 の feature プロファイル構造（要点）:

- `default` = `2d` + `3d` + `ui` + `audio`
- `2d` = `default_app` + `default_platform` + `2d_bevy_render` + `scene` + `picking`
- `default_platform` は `default_font`・`webgl2`・multi_threaded 等を内包
  （→ **2D プロファイルだけで Web レンダリングと内蔵フォントが入る**）
- `2d_bevy_render` = `2d_api` + `bevy_render` + `bevy_core_pipeline`
  + `bevy_post_process` + `bevy_sprite_render` + `bevy_gizmos_render`
- `audio` = `bevy_audio` + `vorbis`
- 画像フォーマット(png/jpeg/webp)はプロファイルに含まれず、**個別 feature** として指定が必要。

削減の本質は、`default` に含まれる **`3d` プロファイル**（bevy_pbr, bevy_light,
atmosphere, ssao, dof, meshlet, solari, bevy_anti_alias 等の巨大な 3D 機構・シェーダ）が
2D ゲームでは完全に不要である点。

## 3. 実施した対策（game_engine/Cargo.toml）

### 3.1 feature の刈り込み

`default-features = false` にし、2D ブロック崩しに必要な機能だけを有効化する。
これで `3d` プロファイルが丸ごと除外される（**削減の本体**）。

```toml
bevy = { version = "0.19.0", default-features = false, features = [
    "2d",     # Camera2d / Sprite / Mesh2d / ColorMaterial + レンダリング。
              # default_platform 経由で default_font と webgl2 も入る。
    "ui",     # スコア表示の Node / Text レイアウト。
    "audio",  # 衝突音(AudioPlayer / .ogg)。vorbis を内包。
    "png",    # デフォルト背景 background.png（画像フォーマットは要明示）。
    "jpeg",   # React が渡す外部背景用。
    "webp",   # 同上。
] }
```

### 3.2 release プロファイルの追加設定

`opt-level = "s"` / `lto = true` / `codegen-units = 1` は元から設定済み。
今回さらに 2 項目を追加した。

```toml
[profile.release]
opt-level = "s"       # サイズ優先。"z" も候補だが cheatbook 曰く実測で決めるべき。
lto = true
codegen-units = 1
panic = "abort"       # パニック巻き戻し機構を削除（Bevy は unwind 不要）→ サイズ減。
strip = true          # シンボル情報を除去。
```

## 4. 効果（計測値）

| 項目 | 値 |
| --- | --- |
| Before | 60,135,378 bytes（約 57.3 MiB） |
| After | 26,402,858 bytes（約 25.1 MiB） |
| 削減量 | -33.7 MB / **-56.1%** |
| ビルド時間 | release + LTO で約 3分46秒 |

## 5. 検証（実ブラウザ・重要）

feature の刈り込みは描画・テキスト・音の欠落を招くリスクがあるため、
「ビルド成功」で終わらせず Playwright（headless chromium + SwiftShader）で
実ブラウザ描画を確認した。手法の詳細は [[20260711_wasm-bevy-browser-verification]]。

確認結果（機能欠落なし）:

- ブロック(Sprite)・ボール(Mesh2d + ColorMaterial)・パドルが正常描画。
- 「Score」テキスト（`bevy_ui` + `default_font`）が正常描画。
- スコア加算も動作。
- パニックや missing-feature 由来のエラーは無し。

## 6. 学び / 今後の伸びしろ（未実施）

- **default features フル有効の 3D 機構が 2D ゲームでは最大の無駄。**
  `default-features = false` + プロファイル指定が桁で効く（今回 -56%）。
- feature 名はバージョンで揺れる。**ビルドエラーを見ながら 1 つずつ足す**のが確実。
- さらに削る候補（いずれも未実施 / トレードオフあり）:
  - `opt-level = "z"`（要実測。"s" より小さくなるとは限らない）。
  - `2d` / `ui` が抱き込む `scene` / `picking` を捨てて granular 指定
    （未使用機能。ただし壊れやすい）。
  - `wasm-opt -Oz`（cargo 外・今回対象外）。
  - nightly の `build-std`（運用が重い）。
