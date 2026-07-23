# `App::add_systems` の第一引数（スケジュールラベル）に何が取れるか — Bevy 0.19

作成日: 2026-07-17
出典: インストール済みクレート `bevy_app 0.19.0` / `bevy_state 0.19.0` / `bevy_ecs 0.19.0` の
実ソース（`~/.cargo/registry`）を一次情報として確認。公式 docs.rs を補助として併記（下部 Sources）。

実装での使用例は [[20260717_game-state-and-clear-notification-impl]]、Bevy States の使い方は
[[20260716_bevy-resource-res-resmut-basics]] も参照。

## 前提: `add_systems` の第一引数は「スケジュールラベル」

Bevy 0.19 の `App::add_systems` のシグネチャは次の通り（`bevy_app 0.19.0/src/app.rs`）。

```rust
pub fn add_systems<M>(
    &mut self,
    schedule: impl ScheduleLabel,                       // ← 第一引数
    systems: impl IntoScheduleConfigs<ScheduleSystem, M>, // ← 第二引数
) -> &mut Self
```

- **第一引数 `schedule`** = **`ScheduleLabel` トレイトを実装した任意の型**。「その system 群を*いつ*
  走らせるか」を指す。`Update` / `Startup` / `OnEnter(...)` などはすべてこのラベル。
- **第二引数 `systems`** = 実行する system 群。`.chain()` / `.run_if(...)` / `.before(...)` /
  `.after(...)` / `.in_set(...)` はこちら（第二引数側）に付ける修飾であって、第一引数ではない。

つまり質問の `add_systems(Update, check_game_clear.run_if(in_state(GameState::Playing)))` は、
**第一引数 = `Update`（スケジュール）**、**第二引数 = `check_game_clear.run_if(...)`（system + 条件）**
という構成。「第一引数に何が取れるか」＝「どんな `ScheduleLabel` が使えるか」であり、以下に列挙する。

## 組み込みスケジュール①: メインループ（`bevy_app` main_schedule.rs）

毎フレーム走る `Main` スケジュールは、内部で次の順に子スケジュールを回す（ソースの `Main` doc と
`MainScheduleOrder` の `labels` から確定）。

| 順 | ラベル | 役割 |
|---|---|---|
| 1 | `First` | フレーム最初に走る |
| 2 | `PreUpdate` | `Update` の前処理。例: OS のキーボード入力イベントを `Messages` に取り込む等、`Update` が使う API を「準備」する |
| 3 | `StateTransition` | 状態遷移を処理（`bevy_state` feature 有効時のみ挿入。`default` に含まれる。後述） |
| 4 | `RunFixedMainLoop` | 経過時間に応じて `FixedMain` を 0〜複数回まわす |
| 5 | `Update` | 大半のゲームプレイロジック。毎フレーム 1 回 |
| 6 | `SpawnScene` | `Update` の後・`PostUpdate` の前。シーン生成向け |
| 7 | `PostUpdate` | `Update` の結果に反応する後処理 |
| 8 | `Last` | フレーム最後に走る |

補足:
- `RunFixedMainLoop` に直接足した system は、他と**並列化されない**点が他スケジュールと異なる（ソース注記）。
- レンダリングは既定ではこのメインスケジュール外（別 `SubApp`）で走る。

## 組み込みスケジュール②: 起動時（1回だけ）

`Startup` 系はアプリ起動時に一度だけ、メインループが始まる前に次の順で走る
（`MainScheduleOrder` の `startup_labels`）。

1. `PreStartup`
2. `Startup` ← 本プロジェクトの `setup` はここ
3. `PostStartup`

## 組み込みスケジュール③: 固定タイムステップ（`FixedMain`）

`RunFixedMainLoop` の中で、消費すべき経過時間が尽きるまで `FixedMain` が繰り返し回る。
その内部順は次の通り。物理・AI・ネットワーク・ゲームルールなど「フレームレートに依らず一定間隔で
回したい」ロジック向け。

1. `FixedFirst`
2. `FixedPreUpdate`
3. `FixedUpdate` ← 固定間隔ゲームプレイロジックの主戦場
4. `FixedPostUpdate`
5. `FixedLast`

## 組み込みスケジュール④: State 関連（`bevy_state` transitions.rs）

`#[derive(States)]` した状態型に対して、遷移に紐づくスケジュールが使える。いずれも
「その system 群を*いつ*走らせるか」を状態遷移のタイミングで指定するラベル。

| ラベル | いつ走るか | 備考 |
|---|---|---|
| `OnEnter(S)` | `State<S>` がその状態に**入った**ときだけ | 同一状態への遷移（identity transition）は無視 |
| `OnExit(S)` | `State<S>` がその状態から**出た**ときだけ | 同一状態への遷移は無視 |
| `OnTransition { exited, entered }` | `exited` から出て `entered` に入る遷移のとき | **`OnExit` の後・`OnEnter` の前**に走る。identity transition でも走る |
| `StateTransition` | 遷移処理そのものを行うスケジュール | 既定で `PreStartup` の前に一度、以降は毎フレーム `PreUpdate` の後に走る |

同一状態遷移での実行順は **`OnExit` → `OnTransition` → `OnEnter`**。本プロジェクトの
`add_systems(OnEnter(GameState::Cleared), on_game_clear)` は、`Cleared` に入った瞬間に一度だけ
`on_game_clear` を走らせる指定になる（[[20260717_game-state-and-clear-notification-impl]]）。

`OnEnter(S)` / `OnExit(S)` はタプル構造体なので `OnEnter(GameState::Cleared)` のように状態値を
渡してインスタンス化したものがラベルになる（値ごとに別スケジュール）。

## 組み込みスケジュール⑤: その他

- `Main` … 上記メインループ全体を回す親スケジュール（通常ここに直接 system は足さない）。
- `FixedMain` … 固定ループ全体の親。
- `SpawnScene` … 上表参照。

## カスタムスケジュールを第一引数にする

組み込み以外に、**自作のラベル型**も第一引数に取れる。`#[derive(ScheduleLabel)]` に加えて
`Clone, Debug, PartialEq, Eq, Hash` が必要（組み込みラベルもこの derive セットで定義されている）。

```rust
use bevy::ecs::schedule::ScheduleLabel;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct MySchedule;

app.init_schedule(MySchedule);            // スケジュールを登録し
app.add_systems(MySchedule, my_system);   // 第一引数に指定できる
// 実行タイミングは自分で world.run_schedule(MySchedule) 等で駆動するか、
// MainScheduleOrder に挿入して既存の流れに組み込む。
```

## 読み方の注意

- 本ノートの一覧・実行順序・doc 説明は、**この環境にインストールされている crate 実ソース
  （`bevy_app 0.19.0` 等）を直接確認した一次情報**。バージョンが変われば増減・順序変更があり得る
  （特に State 周りは Bevy のバージョン間で再編が入りやすい）。
- 下部 Sources の docs.rs は公開版の同 API ドキュメント。crate ソース内の doc リンクが指す先でもある。
  `latest` を指すため、参照時は 0.19 と表記が一致するか確認すること。
- `StateTransition` は `bevy_state` feature（`default` に含まれる）が有効な場合のみメインループに
  挿入される。本プロジェクトは `default-features = false` だが、`2d` プロファイル経由で State が
  使える構成になっている（実際に `init_state` / `OnEnter` がビルド・動作している）。

## Sources

- [bevy_app::main_schedule（Main / Update / Startup 等の定義と順序） — docs.rs](https://docs.rs/bevy_app/0.19.0/bevy_app/main_schedule/index.html)
- [Struct bevy_app::Main（メインスケジュールの実行順序の説明） — docs.rs](https://docs.rs/bevy/latest/bevy/prelude/struct.Main.html)
- [Struct bevy::prelude::OnEnter — docs.rs](https://docs.rs/bevy/latest/bevy/prelude/struct.OnEnter.html)
- [Struct bevy::prelude::OnExit — docs.rs](https://docs.rs/bevy/latest/bevy/prelude/struct.OnExit.html)
- [Struct bevy::prelude::OnTransition — docs.rs](https://docs.rs/bevy/latest/bevy/prelude/struct.OnTransition.html)
- [Struct bevy::prelude::StateTransition — docs.rs](https://docs.rs/bevy/latest/bevy/prelude/struct.StateTransition.html)
- [Trait bevy_ecs::schedule::ScheduleLabel — docs.rs](https://docs.rs/bevy_ecs/0.19.0/bevy_ecs/schedule/trait.ScheduleLabel.html)
- 一次確認: `~/.cargo/registry/src/index.crates.io-*/bevy_app-0.19.0/src/main_schedule.rs`, `bevy_state-0.19.0/src/state/transitions.rs`, `bevy_app-0.19.0/src/app.rs`
