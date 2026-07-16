# 画像のアスペクト比を維持する（contain フィット）と余白を黒にする（レターボックス）

日付: 2026-07-15

盤面（900×600）と画像の縦横比が違っても **引き伸ばさない**（アスペクト比を保つ）ようにし、
比率が合わずにできる余白は **黒** で塗る、という対応の記録。背景画像・ブロック画像の両方に
同じ考え方を適用している。

ブロックの画像描画そのものは [[20260715_brick-image-rendering]]、コードの配置は
[[20260715_main-rs-module-split]] を参照。

---

## 1. きっかけと要件

背景に縦長の写真（例: `Ameca_robot.jpg` = 962×1080）を指定したところ、従来は盤面
（900×600）いっぱいに **引き伸ばして** いたため人物が横に潰れた。要件:

> ウィンドウ（盤面）の比率と画像の比率が違っても、アスペクト比は変えないでほしい。
> 当然できる余白は黒にしてほしい。

## 2. contain フィット（比率維持で内接）

「引き伸ばさずに枠へ収める」= CSS の `object-fit: contain` と同じ。枠に対して縦横それぞれの
拡大率を出し、**小さい方**を採用すれば画像全体が枠に収まり、余った方向に余白ができる。

```rust
// rendering.rs
fn contain_fit(content: Vec2, container: Vec2) -> Vec2 {
    let scale = (container.x / content.x).min(container.y / content.y);
    content * scale
}
```

例: `Ameca_robot.jpg`（962×1080）を 900×600 に contain → `scale = min(900/962, 600/1080)
= min(0.936, 0.556) = 0.556` → 表示は約 **534×600**。左右に黒帯（各 ~183px）が出る。

## 3. 余白を黒にする＝画面クリア色を黒に

余白（レターボックス）は「何も描かれていない領域」なので、**画面のクリア色**がそのまま見える。
そこでクリア色を黒にする。

```rust
// main.rs
.insert_resource(ClearColor(Color::BLACK))
```

これに伴い、旧来の「画像が無いときのフォールバック色」だった `BACKGROUND_COLOR`
（薄いグレー）は不要になり削除した。

## 4. 背景画像への適用

背景は contain フィットした寸法を `custom_size` に入れて中央に置くだけ:

```rust
// setup.rs（要点）
let (background_handle, background_size) = match background_override.0.take() {
    Some(image) => {
        let image_size = Vec2::new(image.width() as f32, image.height() as f32);
        (images.add(image), contain_fit(image_size, BACKGROUND_SIZE))
    }
    // AssetServer 経由は起動時点で寸法が未確定 → 従来どおり全面に引き伸ばし
    None => (asset_server.load(BACKGROUND_IMAGE_PATH), BACKGROUND_SIZE),
};
commands.spawn((
    Sprite { image: background_handle, custom_size: Some(background_size), ..default() },
    Transform::from_xyz(0.0, 0.0, -10.0),
));
```

### 注意：寸法が分かるのは「React が Image を渡したとき」だけ

`image.width()/height()` はデコード済み `Image` から取れる。React 注入経路
（[[20260711_react-to-bevy-background-injection]]）はバイト列をその場でデコードするので寸法が分かる。
一方 `AssetServer::load()` は **非同期ロード**で、`setup` 実行時点ではまだ寸法が確定していない。
そのためデフォルト背景パスのフォールバックだけは従来どおり全面引き伸ばしのままにしている
（実運用は常に React 注入なので実害なし。厳密に対応するならロード完了後にサイズを反映する
別システムが必要）。

## 5. ブロック画像への適用（余白＝黒ブロック）

ブロック画像も同じ内接矩形に対して切り出す。内接矩形の **外** にかかるブロックは画像を貼らず
黒で描く（`brick_image_rect` が `None` を返す → `Color::BLACK` のスプライト）。これにより
「画像の余白＝黒」という背景側のルールとブロック側の見た目が一致する。

- 画像が盤面と同じ 3:2（例: `sample_grid.png` = 900×600）なら内接矩形＝盤面全体になり、
  黒ブロックは発生しない。
- 画像が縦長／横長なら、内接矩形の外に置かれたブロックが黒くなる。

具体的な写像式は [[20260715_brick-image-rendering]] の `brick_image_rect` を参照。

## 6. まとめ（設計上の対応表）

| 対象 | 引き伸ばし回避 | 余白の黒 |
|---|---|---|
| 背景画像 | `custom_size = contain_fit(image, arena)` | `ClearColor(BLACK)`（描かれない領域に出る） |
| ブロック画像 | 内接矩形に対して `rect` で切り出し | 内接矩形外のブロックを `Color::BLACK` で描画 |

## 7. 関連ファイル

- `game_engine/src/main.rs` … `ClearColor(Color::BLACK)`
- `game_engine/src/setup.rs` … 背景の contain フィット
- `game_engine/src/rendering.rs` … `contain_fit` / `brick_image_rect`（内接矩形と黒判定）
- `game_engine/src/config.rs` … `BACKGROUND_SIZE`（アリーナ寸法）
