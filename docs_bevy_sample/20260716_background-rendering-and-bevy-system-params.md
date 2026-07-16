# 背景画像はどこで描かれる？ と Bevy の system 引数（Commands / Res / ResMut）入門

日付: 2026-07-16

「背景画像のレンダリング処理が見当たらない」「`setup` の引数（`Commands` や `ResMut<...>`）が
何なのか分からない」という疑問を解いた記録。Bevy 初学者向けに、**描画の実体がどこにあるか**と
**system の引数がなぜ勝手に埋まるのか**を整理する。

前提として、背景画像の React → Bevy への注入経路は
[[20260711_react-to-bevy-background-injection]] / [[20260711_react-to-bevy-init-params]]、
アスペクト比維持は [[20260715_aspect-ratio-and-letterbox]] を参照。

---

## 1. 背景画像の「レンダリング」はどこにあるか

結論: **`rendering.rs` ではなく `setup.rs` の `setup()` の中**でスプライトを `spawn` している。

`rendering.rs` は名前に反して汎用の描画モジュールではなく、**ブロック（brick）描画専用**の
ヘルパー（`spawn_brick` / `brick_image_rect` / `contain_fit`）。背景は「起動時に 1 枚
スプライトを置くだけ」で状態による描き替えが不要なので、初期化をまとめる `setup.rs` に同居している。
だから「rendering」という名前でファイルを探すと見つからない。

背景に関する処理は 3 ファイルに分かれている:

| やること | 場所 |
|---|---|
| デフォルトパス・サイズの定義 | `config.rs`（`BACKGROUND_IMAGE_PATH` / `BACKGROUND_SIZE`） |
| React からの画像受け取り（Web 専用） | `injection.rs` の `injected_background_image()` → `main.rs` で Resource 登録 |
| **実際の描画（spawn）** | **`setup.rs` の `setup()` 内** |

Bevy では「スプライトを `spawn` する」＝「そのエンティティを毎フレーム自動で描く」という意味。
背景は setup で 1 回置けば以降ずっと表示され続けるので、描画ロジックが独立して見えない。

## 2. setup.rs の背景生成コードの逐語解説

```rust
let (background_handle, background_size) = match background_override.0.take() {
    Some(image) => {
        let image_size = Vec2::new(image.width() as f32, image.height() as f32);
        (images.add(image), contain_fit(image_size, BACKGROUND_SIZE))
    }
    None => (asset_server.load(BACKGROUND_IMAGE_PATH), BACKGROUND_SIZE),
};
commands.spawn((
    Sprite {
        image: background_handle,
        custom_size: Some(background_size),
        ..default()
    },
    Transform::from_xyz(0.0, 0.0, -10.0),
));
```

やっていることは **「使う画像ハンドルと表示サイズを決めて、最背面スプライトとして spawn する」** だけ。

- `background_override.0` は `Option<Image>`（React が画像を渡せば `Some`、無ければ `None`）。
- `.take()` = 中身を取り出して元を `None` にする。`Image` は重くコピー不可なので所有権ごと抜く。
- `match` の結果は `(ハンドル, サイズ)` のタプルで、それを 2 変数に分解代入している。

### `Some(image)` = React が画像を渡してきた

1. `image.width()/height()` で画像の**実ピクセル寸法**を取得（`as f32` で浮動小数へ）。
2. `images.add(image)` … デコード済み `Image` を **アセット倉庫 `Assets<Image>` に登録**し、
   軽い**ハンドル（参照 ID）**を得る。以降は本体でなくハンドルで扱う。
3. `contain_fit(image_size, BACKGROUND_SIZE)` … アリーナ枠に**比率維持で収めた表示サイズ**を計算。

→ 画像を渡された場合だけ寸法が分かるので **引き伸ばさず比率維持**できる。

### `None` = 差し替え無し（デフォルト画像）

- `asset_server.load(BACKGROUND_IMAGE_PATH)` … ファイルから**非同期ロード**。
- サイズは `BACKGROUND_SIZE`（アリーナ全体）をそのまま使う。

**比率維持しない理由**: `load` は非同期で、この時点ではまだ画像が読めておらず寸法が未確定。
だから比率計算ができず、アリーナ全体に引き伸ばす方式にフォールバックする。この非対称性
（React 経由=寸法即判明 / ファイルロード=寸法未確定）が分岐の本質。

### spawn 部分

- `commands.spawn((Sprite {...}, Transform {...}))` … `Sprite` と `Transform` の 2 コンポーネントを
  持つエンティティ（背景）を 1 個生成。Bevy が毎フレーム自動描画する。
- `custom_size: Some(background_size)` … 画像本来の寸法を無視して表示サイズを強制指定。
  未指定だと画像のピクセル寸法で描かれてしまう。
- `Transform::from_xyz(0.0, 0.0, -10.0)` … 中央配置＋ **z=-10 で最背面**（壁・ブロック・ボールは z≧0）。

## 3. Bevy の system 引数とは（Commands / Res / ResMut）

`setup` の引数はこうなっている:

```rust
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut background_override: ResMut<BackgroundOverride>,
    mut brick_layout_override: ResMut<BrickLayoutOverride>,
    mut brick_image_override: ResMut<BrickImageOverride>,
    asset_server: Res<AssetServer>,
) { ... }
```

### 大前提: この関数は「自分で呼ばない」

`main.rs` で `.add_systems(Startup, setup)` と**登録**しておくと、Bevy が起動時に呼び出す。
その際 **引数の型を見て、対応するものを Bevy が自動で用意して渡す**（＝依存性注入）。
だから引数リストは「実行に必要なものの注文票」。`setup(...)` と手で呼ぶ箇所はどこにも無い。

### 引数の 3 種類

| 引数 | 型の種類 | 役割 |
|---|---|---|
| `commands` | `Commands` | エンティティの生成(spawn)・削除(despawn) の命令キュー |
| `meshes` | `ResMut<Assets<Mesh>>` | 形状データの倉庫（書込可） |
| `materials` | `ResMut<Assets<ColorMaterial>>` | 色マテリアルの倉庫（書込可） |
| `images` | `ResMut<Assets<Image>>` | 画像の倉庫（`images.add()` で使用・書込可） |
| `background_override` 他 | `ResMut<自作Resource>` | React 由来データを受け取る（`injection.rs` 定義、`main.rs` で `insert_resource`） |
| `asset_server` | `Res<AssetServer>` | ファイルパスから読み込む係（`.load()`・読取専用） |

- **Resource** = 世界に 1 つだけ存在する共有データ。
- **`Res<T>`** = 読み取り専用、**`ResMut<T>`** = 書き換え可。`asset_server` は読むだけなので `Res`、
  倉庫や override は中身を変えるので `ResMut`。
- `Res` / `ResMut` を区別するのは、Bevy が「誰が何を書き換えるか」を把握して
  **システムを安全に並列実行する**ため（同じ Resource を書く 2 system は同時に走らせない）。
- `mut` は Rust の「後で書き換える変数」の印。`commands.spawn()` や `images.add()` のように
  中身を変えるものに付く。`asset_server` は `.load()` で読むだけなので `mut` 無し。

## 4. 関連ファイル

- `game_engine/src/config.rs` … `BACKGROUND_IMAGE_PATH` / `BACKGROUND_SIZE`
- `game_engine/src/injection.rs` … `injected_background_image()` / `BackgroundOverride`
- `game_engine/src/main.rs` … `insert_resource(...)` で Resource 登録・`add_systems(Startup, setup)`
- `game_engine/src/setup.rs` … `setup()`。背景スプライトの spawn（レンダリングの実体）
- `game_engine/src/rendering.rs` … ブロック描画専用ヘルパー（背景はここには無い）
