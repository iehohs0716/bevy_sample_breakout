# Bevy 設計方針：ゲーム「起動時」と「リスタート時」の State は分ける

日付: 2026-07-23

ブロック崩しに「ライフ 0 で敗北 → 再スタート」を入れる過程で得た設計方針の記録。
結論を先に言うと:

> **起動時（初期状態）に使う State と、リスタート時に使う State は、なるべく同じにしない。
> さらに、分けたリスタート用 State から起動用 State へ「戻して合流」もさせない。**

これは好みの問題ではなく、Bevy の **「初期状態の `OnEnter` は `Startup` より前に走る」** という仕様から来る、実害を避けるための方針。以下、なぜそうなるかを順に。

関連: [[20260717_game-state-and-clear-notification-impl]] / [[20260717_bevy-add-systems-schedule-labels]] / [[20260716_bevy-resource-res-resmut-basics]]

---

## 1. 前提：Bevy の起動順序（ここが肝）

Bevy の `States` は状態遷移を `StateTransition` というスケジュールで処理する。その登録が
`bevy_state-0.19.0/src/app.rs:336` にこうある:

```rust
schedule.insert_startup_before(PreStartup, StateTransition);
```

`insert_startup_before(PreStartup, StateTransition)` = 「`StateTransition` を `PreStartup` の**前**に差し込む」。
`Startup` 系は `PreStartup → Startup → PostStartup` の順なので、実際の起動順序は:

```
① StateTransition   ← 初期状態の OnEnter がここで 1 回走る
② PreStartup
③ Startup           ← 普通の初期化 system（setup 等）はここ
④ PostStartup
⑤ 毎フレームの Update ループ開始
```

**重要**: 一番最初の状態（`#[default]` を付けた State）の `OnEnter` は、`setup`（`Startup`）より
**前**に 1 回走る。「初期化 system が世界を用意してから状態に入る」という直感とは**逆**。

## 2. やってしまった設計ミス（相乗り）

「初回配置」と「再スタート時の配置」を 1 か所にまとめたくて、初期状態を `GameStart` にしたうえで、
盤面リセット（ブロック再配置・ボール初期化）を `OnEnter(GameStart)` の `reset_game` に置いた。
配置情報は `setup`（`Startup`）が作る `GameAssets` リソースから読む設計:

```rust
// setup（Startup）で作る
commands.insert_resource(GameAssets { brick_layout, brick_image });

// reset_game を OnEnter(GameStart) に登録し、GameAssets から spawn する
fn reset_game(game_assets: Res<GameAssets>, /* ... */) { /* spawn */ }
```

これは **起動と同時に panic** する:

```
Parameter `...` failed validation: Resource does not exist
```

理由は 1 章の順序。`reset_game`（`OnEnter(GameStart)`）は `setup`（`Startup`）**より前**に走るので、
その時点で `GameAssets` はまだ存在しない。Bevy は system 実行前に引数（`Res<GameAssets>`）の存在を
検証し、無ければ panic する。**「箱を作る人（setup）より、箱を使う人（reset_game）が先に呼ばれた」**
という順序事故。

## 3. 臭い回避策（ガードで握りつぶす）

その場しのぎに、依存を必須の `Res` から `Option<Res>` に緩め、無ければ即 return した:

```rust
fn reset_game(game_assets: Option<Res<GameAssets>>, ball: Query<..., With<Ball>>, /* ... */) {
    let Some(game_assets) = game_assets else { return; };   // 早すぎる初回発火を握りつぶす
    let Ok((mut transform, mut velocity)) = ball.single_mut() else { return; };
    // ...
}
```

動きはするが、**「この system は起動直後に一度、無意味に呼ばれて空振りする」前提を各所に散らかす**。
Ball も `Single` だと同じ理由で落ちるので `Query` + `single_mut()` に逃がす…と、本質でない防御コードが
増えていく。臭い。原因（State の相乗り）に手を付けていないから。

## 4. 正しい設計：State を分ける

初期状態 `GameStart` とは別に、再スタート専用の `GameRestart` を用意する。

```rust
pub enum GameState {
    #[default]
    GameStart,   // 起動直後のクリック待ち。OnEnter では何もしない
    Playing,
    Cleared,
    GameOver,
    GameRestart, // 敗北後のクリック待ち。OnEnter で reset_game が盤面を作り直す
}
```

- 初回の盤面は `setup`（`Startup`）が作る。
- `reset_game` は `OnEnter(GameRestart)` にだけ登録する。
- `GameRestart` は **`GameOver` 経由（＝起動よりずっと後）にしか入らない**。だから `reset_game` が
  走る時には `GameAssets` も `Ball` も必ず存在する。1 章の「Startup より前に走る」問題が原理的に起きない。

結果、3 章のガードは全部消えて、依存を素直に書ける:

```rust
fn reset_game(
    game_assets: Res<GameAssets>,                         // Option 不要
    ball: Single<(&mut Transform, &mut Velocity), With<Ball>>,  // Single で OK
    /* ... */
) { /* リセット処理だけ */ }
```

**「初期状態の OnEnter で世界を触らない」**（＝起動用 State に初期化ロジックを相乗りさせない）だけで、
順序事故もガードも消える。これが方針の中身。

## 5. 追加の落とし穴：分けたのに「戻して合流」させない

State を分けても、`reset_game` の最後でこう書くと台無しになる:

```rust
// ❌ せっかく分けた GameRestart を、また起動用の GameStart に合流させている
next_state.set(GameState::GameStart);
```

これをやると「再スタート後の待機」がまた `GameStart` に依存する形になり、**分離した意味が消える**
（将来 `GameStart` の OnEnter に初期化を足したら、再スタート経路でも実行されて再び事故る余地が戻る）。

正解は、**`GameRestart` をそのまま「敗北後のクリック待ち」状態として完結させる**こと。共通で必要な
「クリックで発射」だけを両状態で動かす:

```rust
// reset_game は状態を変えない（GameRestart のまま待つ）

// クリック発射だけ GameStart / GameRestart 両方で動かす
app.add_systems(
    Update,
    launch_ball_on_click
        .run_if(in_state(GameState::GameStart).or_else(in_state(GameState::GameRestart))),
);
```

`GameStart`＝初回待ち、`GameRestart`＝敗北後待ち。**互いに遷移し合わない**まま、共通の振る舞い
（クリック→`Playing`）だけを共有する。これで「起動時」と「再スタート時」が最後まで別物として保たれる。

## 6. 一般化した設計指針

1. **初期状態（`#[default]`）の `OnEnter` に、`Startup` が用意するリソース/エンティティを前提とする
   処理を置かない。** 初期状態の OnEnter は `Startup` より前に 1 回走る。
2. **「初回セットアップ」は `Startup`（`setup`）に、「再実行が要る処理」は専用 State の `OnEnter` に。**
   両者を同じ State に相乗りさせない。
3. **専用 State（例: `GameRestart`）を作ったら、それを起動用 State に戻して合流させない。** 共通で
   必要な振る舞いは `run_if(... .or_else(...))` で複数 State に効かせて共有する。
4. `Option<Res<T>>` や `Query` + 早期 return で「まだ準備できてない発火」を握りつぶしたくなったら、
   それは **State 設計が相乗りしているサイン**。ガードで隠す前に State を分けられないか検討する。

## 7. まとめ

- Bevy の初期状態 `OnEnter` は `Startup` より前に走る、という仕様がすべての出発点。
- 起動用 State に初期化を相乗りさせると「リソース未生成で panic」→ ガードだらけになる。
- 起動用（`GameStart`）と再スタート用（`GameRestart`）を分け、かつ**再スタート用を起動用に戻さない**
  ことで、順序事故もガードも消え、各 State の役割が 1 つに保たれる。
