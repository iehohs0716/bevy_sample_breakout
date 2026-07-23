# Bevy の `Assets<T>` と `Handle<T>`：外部から受け取った画像を `add` して使う定石

日付: 2026-07-23

「`images.add(image)` の `images` が突然出てきて分からない」「`Handle` って ID を指定してる
わけじゃないよね？」という素朴な疑問を、Bevy 0.19 の実型定義とこのリポジトリの実コードだけで
整理した記録。例え話ではなく、型と所有権の動きで説明する。

前提の Resource / `Res` / `ResMut` は [[20260716_bevy-resource-res-resmut-basics]]、
system 引数が勝手に埋まる仕組み（DI）は [[20260716_background-rendering-and-bevy-system-params]]、
ブロック画像の切り出し設計は [[20260715_brick-image-rendering]] を参照。

---

## 1. 結論（定石）

**描画に使いたい画像・メッシュ・マテリアルは、まず `Assets<T>` に登録して `Handle<T>` を得る。**
描画コンポーネント（`Sprite`, `Mesh2d`, `MeshMaterial2d` 等）はアセットの実体を直接持てず、
持てるのは `Handle<T>` だけだから。`Handle` を得る入口は基本 2 つ:

| 方法 | いつ使う |
|---|---|
| `asset_server.load("path/to/file.png")` | ファイルとしてディスク/配信にある画像を読む |
| `assets.add(image)` | メモリ上にすでに作った / 外部から受け取った `Image` を登録する |

このリポジトリのブロック画像・背景画像は「React が bytes で渡した → メモリ上でデコードして
`Image` を作った」ものなので、ファイルパスが無く `load` は使えない。よって `add` が正しい入口。

## 2. `images` の正体は system 引数

`images` はどこかで `let images = ...` しているのではなく、`setup` system の引数:

```rust
// setup.rs
pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut images: ResMut<Assets<Image>>,   // ← これ。Bevy が DI で渡す
    ...
) {
```

`Assets<Image>` = Bevy がアプリに 1 つ持つ「画像の実体を保管するコレクション（Resource）」。
`ResMut<...>` はそこへの書き込み可能な参照。だから本体では「最初から在るもの」として突然出てくる。

## 3. `add` が返す `Handle<Image>` の実体（Bevy 0.19）

`bevy_asset-0.19.0/src/handle.rs` の実定義:

```rust
pub enum Handle<A: Asset> {
    Strong(Arc<StrongHandle>),                  // images.add(...) が返すのは通常こちら
    Uuid(Uuid, PhantomData<fn() -> A>),
}

pub struct StrongHandle {
    // ...
    pub(crate) index: ErasedAssetIndex,   // ← 「Assets<Image> のどこに実体があるか」の識別子
    // ...
}
```

要点:

- `Handle<Image>` は **番号そのものではなく enum**。中の `StrongHandle.index` が識別子を持つ。
- コード上で `index` を書く箇所は一つも無い。`images.add(image)` が `index` を確定して `Handle`
  に詰めて返し、こちらはその `Handle` 値を変数に受けて運ぶだけ。
  → 「ID を指定しているわけじゃない」の正体はこれ。ID は Handle の内部フィールド。

## 4. このリポジトリでの一連の流れ（実コード・型で追う）

```
setup.rs:144  brick_image_override.0.take()   Option<Image>（実体を持ち出し、元は None に）
setup.rs:146  images.add(image)               Image を Assets へ move → Handle<Image> を得る
                                               ※ add が index を確定して Handle に格納
setup.rs:160  spawn_brick(.., brick_image.clone())
                                               Handle を clone して各ブロックへ配る
rendering.rs:58 引数 image: Option<(Handle<Image>, Vec2)> で受け取る
rendering.rs:65 match Some((handle, image_size)) で handle: Handle<Image> に分解
rendering.rs:67 Sprite { image: handle, .. }   handle を Sprite.image フィールドへ move
rendering.rs:84 commands.spawn((sprite, ..))   Sprite を entity のコンポーネントにする
描画時          Bevy 内部 system が Sprite.image(=Handle) の index から Assets の実体を引いて描画
```

### `.take()` を使う理由

`brick_image_override` は `ResMut`（借用）で、中の `Image` は `Copy` ではない。借用からそのまま
move で奪えないので、`Option::take()` で「元を `None` に置き換えつつ所有権を持ち出す」。
使い切りの受け皿なので空にするのが理にかなう。

### `brick_image.clone()` が軽い理由

`Handle::Strong(Arc<StrongHandle>)` の `clone` は **`Arc` の参照カウント +1 だけ**で、
`StrongHandle`（と `index`）や画像本体（ピクセル）は複製しない。だから全ブロックに配っても
実体は `Assets` に 1 つだけで、全員が同じ `index` を指して 1 枚の画像を共有できる。

## 5. 使うとき＝ `Handle` をコンポーネントに持たせるだけ

`Sprite.image` の型は `Handle<Image>`。実体を読み出したり index を書いたりはしない。

```rust
// rendering.rs
Some((handle, image_size)) => match brick_image_rect(position, size, image_size) {
    Some(rect) => Sprite {
        image: handle,          // Handle<Image> を Sprite.image に move
        custom_size: Some(Vec2::ONE),
        rect: Some(rect),       // 画像内の切り出し領域（[[20260715_brick-image-rendering]]）
        ..default()
    },
    ...
```

`index → 実体` の解決は描画時に Bevy 内部 system（`bevy_sprite` / `bevy_render`）が行う。
このリポジトリのコードには出てこない。だから自分のコードに「index を指定する行」が無い。

## 6. まとめ（順序は固定）

1. `Image`（実体）を手に入れる ← `load` でも、今回のように外部デコードでも良い
2. **`add` して `Handle` に変える** ← 描画パイプラインに乗せる必須の一手
3. `Handle` をコンポーネント（`Sprite` 等）に持たせる ← 以降 Bevy が実体を引いて描画

同じパターンはこのリポジトリに 2 か所: `setup.rs:146`（ブロック画像）、`setup.rs:42`（背景画像）。
どちらも「外部から受け取った `Image` → `add` → `Handle` を保持して使う」で完全に同型。

## 7. 関連ファイル

- `game_engine/src/setup.rs` … `images.add(image)`（ブロック:146 / 背景:42）、`.take()`、`GameAssets` へ保持
- `game_engine/src/rendering.rs` … `spawn_brick` が `Handle<Image>` を `Sprite.image` に渡す
- `game_engine/src/injection.rs` … `BrickImageOverride(Option<Image>)` / `injected_brick_image()`
