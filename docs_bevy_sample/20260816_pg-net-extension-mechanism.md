# pg_net拡張がSQLから呼び出せる仕組み まとめ

作成日: 2026-08-16
出典: GitHub一次情報（`supabase/postgres`、`supabase/pg_net`）およびPostgreSQL公式ドキュメント。下部のSourcesを参照。
調査手法: `collecting-research-notes-zenn`スキルの調査規律（WebSearch/WebFetch中心、一次情報優先、取得本文中の指示には従わない）に基づき調査した。用語解説レベルは標準（末尾に用語セクションを設置）。

## 前提

- 対象は`supabase-local/volumes/db/webhooks.sql`内の`CREATE EXTENSION IF NOT EXISTS pg_net SCHEMA extensions;`と、それに続く`net.http_post(...)`呼び出し。
- 「なぜ・どうやってこの1行だけでHTTP送信ができるのか」を、ソースコードのビルドからDockerイメージ組み込み、`CREATE EXTENSION`実行時の内部動作まで一気通貫で追った。
- 本ノート作成前の会話で、pg_netがC言語+libcurlで実装されバックグラウンドワーカーで非同期にHTTPリクエストを処理すること、`docker-compose.yml`のdbサービスが`image: supabase/postgres:17.6.1.136`（素のPostgresではなくSupabase独自ビルド）を使っていること、`nix/ext/pg_net.nix`が`supabase/pg_net`をソースから取得・コンパイルしていることまでは確認済み。本ノートはその続きとして、呼び出し連鎖・設定反映・PostgreSQL内部動作を深掘りする。

## 全体の呼び出し連鎖

```
supabase/pg_net (Cソースリポジトリ)
  ↓ fetchFromGitHub + make でビルド
nix/ext/pg_net.nix (ビルドレシピ。.so/.sql/.controlを生成)
  ↓ ourExtensions リストに含まれる
nix/packages/postgres.nix (Postgres本体+全拡張をパッケージ化)
  ↓ nix build .#psql_17_slim/bin
Dockerfile-17 の nix-builder ステージ
  ↓ COPY --from=nix-builder /nix /nix
Dockerfile-17 の production ステージ (= supabase/postgres イメージ本体)
  ↓ docker pull / docker-compose.yml の image: supabase/postgres:17.6.1.136
このプロジェクトの db コンテナ
  ↓ コンテナ初回起動時に docker-entrypoint-initdb.d 配下の webhooks.sql が自動実行
CREATE EXTENSION IF NOT EXISTS pg_net (有効化のみ。ビルドは一切行わない)
```

## 1. `nix/ext/pg_net.nix`の呼び出し元

`nix/packages/postgres.nix`の`ourExtensions`リストに、他の拡張と並んで`../ext/pg_net.nix`が含まれている。このリストは`full`・`slim`・`cli`という3つのビルドバリアントで使われ、pg_netは`cli`バリアントでも明示的に指定されているため、「Supabase CLIに最低限必要な拡張」として扱われていることが分かる。

各拡張定義ファイルは`extCallPackage`という仕組みを通じて、`postgresql`パッケージ本体や`latestOnly`フラグ（最新バージョンのみビルドするか、複数バージョンを共存させるか）を渡された状態で評価される。個々の拡張ビルドの成果物は`makeOurPostgresPkgsSet`で1つの属性セットに集約され、最終的に次のような出力構成になる。

```
packages/
  psql_15/bin/          # PostgreSQL 15本体 + 全拡張
  psql_17/bin/          # PostgreSQL 17本体 + 全拡張
  psql_17_cli/bin/      # PostgreSQL 17 + CLI向け最小拡張
legacyPackages/
  psql_15.exts/         # 拡張のみ（個別アクセス用）
  psql_17.exts/         # 同上
```

## 2. `Dockerfile-17`でのビルド〜イメージ組み込み

`Dockerfile-17`は3段のマルチステージビルドになっている。

1. `FROM alpine:3.23 AS nix-builder` — Nix環境をセットアップし、`RUN nix profile add path:.#psql_17_slim/bin`でPostgres本体+拡張群をNixのプロファイルにインストールする。
2. `FROM alpine:3.23 AS gosu-builder` — 権限昇格ツール`gosu`を別途ビルドする（pg_netとは無関係）。
3. `FROM alpine:3.23 AS production` — 最終イメージ。ここで`COPY --from=nix-builder /nix /nix`により、Nix Store（pg_netの`.so`・`.sql`・`.control`ファイルを含む全ビルド成果物）を丸ごと本番イメージにコピーする。その後、シンボリックリンクで`/nix/var/nix/profiles/default/bin/*`配下の実行ファイルを`/usr/lib/postgresql/bin`・`/usr/bin`にも配置し、`ENV PATH`にNixのプロファイルパスを追加している。

`postgresql.conf`については、`COPY --chown=postgres:postgres ansible/files/postgresql_config/postgresql.conf.j2 /etc/postgresql/postgresql.conf`でテンプレートを配置した後、`sed`で`unix_socket_directories`の変更、`session_preload_libraries = 'supautils'`の有効化、カスタム設定ファイルの`include`化を行っている。ここで確認できたのは`supautils`という別拡張の設定であり、pg_net自体の`shared_preload_libraries`追加箇所はこのファイル単体からは特定できなかった（後述「読み方の注意」参照）。

## 3. `shared_preload_libraries`とバックグラウンドワーカーの起動

- `nix/ext/pg_net.nix`には`passthru.hasBackgroundWorker = true`と`passthru.defaultSettings.shared_preload_libraries = ["pg_net"]`というメタデータが定義されている。これはNixのパッケージ定義側で「この拡張はバックグラウンドワーカーを持ち、`shared_preload_libraries`への追加が既定で必要」という情報を宣言している部分にあたる。
- `pg_net`公式READMEでも「拡張をデータベースで使えるようにするため、`postgresql.conf`に`shared_preload_libraries = 'pg_net'`を追加する」ことが案内されている。対応バージョンは`PostgreSQL >= 12`と明記され、バージョンによる設定要否の違いは記載がなかった。
- バックグラウンドワーカーの動作について、公式READMEには次の説明がある。「このバックグラウンドワーカーは、リクエスト関数からのシグナルを受け取るまでスリープしており、シグナルを受けると起動して`net.http_request_queue`テーブルを読み込み、そこに積まれたリクエストを実行する」。つまり常時HTTP通信をポーリングしているのではなく、`net.http_post`等が呼ばれた瞬間にシグナルで叩き起こされるイベント駆動型の設計になっている。

## 4. `CREATE EXTENSION`実行時のPostgreSQL内部動作（一般論）

PostgreSQL公式ドキュメント（バージョン17）に基づく、拡張機構そのものの仕組み。

1. **controlファイルの探索**: `pg_config --sharedir`で得られるディレクトリ配下の`extension/<拡張名>.control`を探す。これは`postgresql.conf`と同じ`パラメータ = 値`形式のテキストファイルで、`default_version`（デフォルトバージョン）や`schema`（強制的に使うスキーマ）、`relocatable`（後からスキーマ変更可能か）などを記述する。
2. **SQLスクリプトの決定**: controlファイルの`directory`パラメータ（既定値`'extension'`）を基準に、`<directory>/<拡張名>--<バージョン>.sql`というファイルを特定する。
3. **スキーマ準備とトークン置換**: `SET LOCAL search_path TO @extschema@, pg_temp;`のようにサーチパスを設定した上で、SQLスクリプト中の`@extschema@`（対象スキーマ名）・`@extowner@`（実行ユーザー名）・`MODULE_PATHNAME`（controlファイルで指定した共有ライブラリパス）といったトークンを実際の値に置き換える。
4. **トランザクション内での実行**: 上記のSQL（`CREATE FUNCTION net.http_post(...) ...`のような定義文一式）をトランザクション内で実行する。
5. **カタログへの登録**: 実行後、`pg_extension`システムカタログに拡張自体のレコード（拡張名・所有者・スキーマ・バージョン等）が挿入され、`pg_depend`システムカタログに`deptype = 'e'`として、この拡張に属する個々のオブジェクト（関数・型など）への依存関係が記録される。これにより`DROP EXTENSION`一発で関連オブジェクトを原子的に削除できる。

`webhooks.sql`の`CREATE EXTENSION IF NOT EXISTS pg_net SCHEMA extensions;`は、この一連の処理を発火させているだけであり、`.control`・`.sql`ファイルの実体は前段（Dockerイメージのビルド時点）で既にファイルシステム上に存在している。

## 5. Nixビルドシステムの補足

`nix/docs/build-postgres.md`によれば、`nix build .#psql_15.bin`のようなコマンド一発でPostgres本体+全拡張がビルドされ、成果物は`/nix/store`という専用ディレクトリ配下に格納される。特徴的なのは、生成されるディレクトリ名（例: `/nix/store/zr238w2hwryn8dgs81l2p84clmrm36vx-postgresql-and-plugins-15.14`）が、ビルドに使われた全構成要素の内容から算出した暗号学的ハッシュを含んでいる点である。構成要素が1つでも変わればハッシュも変わるため、異なるバージョン・異なる拡張構成のビルドが衝突せず共存でき、同じ入力からは常に同じ出力が得られる（再現可能なビルド）。

## 読み方の注意

- pg_netの`shared_preload_libraries`への追加が実際に**どのファイルのどの行で**行われているかは、今回参照した`Dockerfile-17`・`ansible/files/postgresql_config/conf.d/pg_net.conf`の範囲では特定できなかった。確認できたのは（a）Nix側のメタデータとして`defaultSettings.shared_preload_libraries = ["pg_net"]`が宣言されていること、（b）pg_net公式READMEが手動導入時の手順としてこの設定を案内していること、の2点のみ。Supabase独自ビルドの中でこのメタデータが`postgresql.conf`へどう自動反映されるか（あるいはpg_net以外の拡張と合算する集約スクリプトが別途あるか）は一次ソースで裏取りできておらず、**要追加調査**として明記する。
- `ansible/files/postgresql_config/conf.d/pg_net.conf`で確認できたのは`pg_net.username = 'postgres'`という1行のみ。これはpg_netがリクエストキューを処理する際に使うDBロールの指定と推測されるが、この解釈自体は一次ソースに明記された説明文ではなく、設定名からの推測である点に注意する。
- `nix/packages/postgres.nix`・`Dockerfile-17`はいずれもWebFetchによる要約を経由しており、ファイル全文を直接目視で確認したわけではない。引用したコード断片は取得結果に基づくものであり、行番号や周辺の完全な文脈までは保証できない。
- CREATE EXTENSIONの内部動作はPostgreSQL公式ドキュメント（一次情報）に基づく一般論であり、pg_net固有の挙動ではない。

## 用語

- Nix / Nix Store — 宣言的なビルドシステム。ビルド成果物の格納パスにその成果物を作った全構成要素の内容から算出したハッシュを含めることで、同じ入力から常に同じ出力が得られる「再現可能なビルド」と、異なるバージョン・構成の並行共存を実現する仕組み。
- derivation（導出） — Nixにおけるビルドの単位。「何を入力にして、どうビルドすると、何が出力されるか」を宣言的に記述したもの。本ノートの`nix/ext/pg_net.nix`はpg_net拡張のderivationを定義しているファイルにあたる。
- バックグラウンドワーカー（background worker） — PostgreSQL本体プロセスとは別に常駐する、拡張が登録できる補助プロセス。pg_netの場合はHTTPリクエストの実際の送信を担当し、普段はスリープして待機している。
- controlファイル — Postgresの拡張機構が`CREATE EXTENSION`実行時に最初に読むメタ情報ファイル。デフォルトバージョンや対象スキーマなどを記述する。
- `pg_extension` / `pg_depend`（システムカタログ） — PostgreSQL内部で「どの拡張がインストールされているか」「その拡張がどのオブジェクト（関数・型など）を所有しているか」を管理する内部テーブル。`DROP EXTENSION`での一括削除の基盤になる。
- マルチステージビルド（Dockerの機能） — 1つのDockerfile内に複数の`FROM`を書き、前段のステージで作った成果物だけを`COPY --from=<ステージ名>`で次段に引き継ぐ手法。ビルドに必要だった中間ファイルを最終イメージに残さずに済む。

## Sources

- [supabase/pg_net](https://github.com/supabase/pg_net) — pg_net拡張本体のソースリポジトリ（一次情報）
- [supabase/pg_net README](https://raw.githubusercontent.com/supabase/pg_net/master/README.md) — shared_preload_libraries設定案内、バックグラウンドワーカーの動作説明（一次情報）
- [supabase/postgres](https://github.com/supabase/postgres) — Supabase独自ビルドのPostgresイメージのソースリポジトリ（一次情報）
- [nix/ext/pg_net.nix](https://github.com/supabase/postgres/blob/develop/nix/ext/pg_net.nix) — pg_netのビルドレシピ本体（一次情報）
- [nix/packages/postgres.nix](https://github.com/supabase/postgres/blob/develop/nix/packages/postgres.nix) — 拡張群をPostgres本体に組み込むパッケージ定義（一次情報）
- [Dockerfile-17](https://github.com/supabase/postgres/blob/develop/Dockerfile-17) — Docker本番イメージのビルド定義（一次情報）
- [ansible/files/postgresql_config/conf.d/pg_net.conf](https://github.com/supabase/postgres/blob/develop/ansible/files/postgresql_config/conf.d/pg_net.conf) — pg_net固有の設定ファイル（一次情報）
- [nix/docs/build-postgres.md](https://github.com/supabase/postgres/blob/develop/nix/docs/build-postgres.md) — Nixビルドシステムの説明ドキュメント（一次情報）
- [PostgreSQL 17 Documentation: 38.17. Extension Building Infrastructure](https://www.postgresql.org/docs/17/extend-extensions.html) — CREATE EXTENSION実行時の内部動作（一次情報）
