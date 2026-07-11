# Bevy Breakout サンプルのセットアップ・トラブルシューティング記録

日付: 2026-07-11

Bevy 公式 Breakout サンプルを `game_engine/` で動かすまでに遭遇した 2 つのエラーと対処のメモ。

---

## 1. mise で cargo をインストールしようとして失敗する

### 症状

```
mise ERROR failed to install cargo@0.98.0
mise ERROR failed to execute command: cargo install cargo@0.98.0 --locked --root ...
mise ERROR No such file or directory (os error 2)
```

### 原因

- `mise install cargo@0.98.0` のように指定すると、mise は `cargo` を **cargo バックエンド**（`cargo install <crate名>` で crate を入れる仕組み）として解釈する。
- そのため内部的に `cargo install cargo@0.98.0 ...` を実行し、「`cargo` という名前の crate」を入れようとする。
- 最後の `No such file or directory (os error 2)` は、その実行に必要な **`cargo` バイナリ自体が PATH に無い**ことを示す（Rust ツールチェイン未インストール状態）。

要するに二重の勘違い:
1. `cargo@x.y.z` は Rust バージョン指定のつもりでも、mise 的には「cargo バックエンドで crate を入れる」指定になる。
2. そもそもその実行に必要な cargo 本体が無い。

### 対処

Rust を mise で管理する場合、指定するツール名は `cargo` ではなく **`rust`**。

```bash
mise use -g rust@latest      # 最新安定版
# または mise use -g rust@1.97 のようにバージョン固定
mise install
cargo --version
```

誤登録された `cargo` エントリが残っていれば削除:

```bash
mise ls
mise rm cargo
```

補足: `cargo:` バックエンドは本来 crate をインストールする用途（例: `mise use -g "cargo:ripgrep@latest"`）。ツールチェイン管理には使わない。

---

## 2. `cargo run` で `error[E0583]: file not found for module 'stepping'`

### 症状

```
error[E0583]: file not found for module `stepping`
  --> src/main.rs:10:1
   |
10 | mod stepping;
   | ^^^^^^^^^^^^^
```

### 原因

- これは Bevy 公式の **Breakout サンプル**の `main.rs` 単体をコピーした状態。
- `main.rs` の `mod stepping;` が `src/stepping.rs`（または `src/stepping/mod.rs`）を要求するが、そのファイルが無い。
- Bevy リポジトリではこの `stepping` は `examples/stepping.rs` という**複数サンプルで共有されるデバッグ用モジュール**に置かれており、`main.rs` 単体には含まれない。
- `stepping` は「1 フレームずつ実行を止めて進める」デバッグ専用機能で、ゲーム本体には必須ではない。

### 対処（stepping を削除する方針）

`game_engine/src/main.rs` から以下 2 箇所を削除:

1. モジュール宣言（旧 10 行目付近）

   ```rust
   mod stepping;
   ```

2. `main()` 内の `SteppingPlugin` 追加ブロック

   ```rust
   .add_plugins(
       stepping::SteppingPlugin::default()
           .add_schedule(Update)
           .at(percent(35), percent(50)),
   )
   ```

これで `cargo build` / `cargo run` が通る。

### 補足

- ビルド時に出る警告
  `ld: __eh_frame section too large (max 16MB) ...`
  はリンカ（macOS）の警告で、動作には影響しない。
- stepping のデバッグ機能を残したい場合は、Bevy 0.19 の `examples/stepping.rs` を `game_engine/src/stepping.rs` として配置し、削除した 2 箇所を復活させる。

---

## 実行環境メモ

- `cargo` はシェルの PATH に出ていないため、実行は次のいずれか:
  - `mise exec -- cargo run`
  - `~/.zshrc` に `eval "$(mise activate zsh)"` を追加して素の `cargo run` を使う
- プロジェクト: `game_engine/`（`Cargo.toml`: `bevy = "0.19.0"`, `edition = "2024"`）
