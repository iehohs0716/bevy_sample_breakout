# Docker バインドマウントのオプション（`:Z`/`:z` 中心）まとめ

作成日: 2026-08-16
出典: Docker公式ドキュメント（一次情報）、Red Hat Developer記事（SELinuxメンテナ組織による解説）。下部の Sources を参照。

## 前提

- きっかけは `docker-compose.yml` の以下の記述にある末尾 `:Z` の意味を確認すること。

  ```yaml
  ./supabase-local/volumes/db/poc_check.sql:/docker-entrypoint-initdb.d/init-scripts/50-poc-check.sql:Z
  ```

- 対象は、SELinux が有効な Linux ホスト（RHEL / CentOS / Fedora 系等）上での Docker バインドマウントである。macOS の Docker Desktop のように SELinux が存在しない環境では later 節の通り無視される。

## `:Z` / `:z` とは何か

SELinux が有効な環境では、ホスト上のファイル・ディレクトリにはそれぞれ SELinux ラベル（コンテキスト）が付与されている。コンテナからバインドマウント経由でホストのパスにアクセスしようとすると、コンテナプロセスのラベルとホスト側ファイルのラベルが一致せず `Permission denied` になることがある。`z` / `Z` オプションは、バインドマウント時にホスト側のラベルを**コンテナ用に付け替える（relabel する）**よう Docker に指示するものである。

Docker 公式ドキュメントの記述：

> SELinux を使用する場合、`z` または `Z` オプションを追加して、コンテナにマウントされるホストファイルまたはディレクトリの SELinux ラベルを変更できます。

| オプション | 意味 | ラベルの種類 |
|---|---|---|
| `z`（小文字） | バインドマウントの内容が**複数のコンテナ間で共有**される | 共有ラベル（shared content label）。MCS 値なしで、SELinux 的にはどのコンテナも read/write 可能になる |
| `Z`（大文字） | バインドマウントの内容が**そのコンテナ専用（private/unshared）** | プライベートラベル（private/unshared label）。そのコンテナだけがアクセスできる |

`docker-entrypoint-initdb.d` のような「特定の1コンテナ（DBコンテナ）だけが読む初期化スクリプト」の用途では、他コンテナと共有する必要がないため `Z`（大文字）が適切な選択になる。

## 重要な注意点（公式ドキュメントの警告）

Docker 公式ドキュメントは次のように明記している。

> これらのオプションを極めて慎重に使用してください。`/home` や `/usr` などのシステムディレクトリをバインドマウントする際に `Z` オプションを使用すると、ホストマシンが動作不可能になり、手動でホストマシンファイルを再ラベル付けする必要がある場合があります。

Red Hat Developer の解説記事も同様に、システムディレクトリ（`/home`、`/etc`、`/var/log` 等）を再ラベリング対象にしないよう明確に警告している。理由は、そのディレクトリを使う他のセキュアなサービスの SELinux ラベルまで書き換わってしまい、動作を妨害する可能性があるため。ボリューム配下のファイル数が多い大規模な場合は、再ラベリングに時間がかかる点や、`udica` のようなツールで専用ポリシーを生成する運用が代替案として挙がっている。

## Docker Compose の `services` での制限

Docker Compose のリファレンスドキュメントには次の記述がある（`docker/docs` リポジトリ `content/reference/compose-file/services.md` より）。

> The SELinux re-labeling bind mount option is ignored on platforms without SELinux.

またバインドマウントを `services`（Swarm 等）として使う場合、SELinux ラベル（`:Z` と `:z`）および `:ro` は無視される、という制限も公式ドキュメントに明記されている。短縮記法 `VOLUME:CONTAINER_PATH:ACCESS_MODE` の `ACCESS_MODE` 部分に `z`（共有）/`Z`（プライベート）を書けるのは通常の `volumes:` 記述の話であり、`services` 経由では効かない点に注意。

## macOS (Docker Desktop) での扱い

SELinux が存在しないプラットフォーム（macOS の Docker Desktop 等）では、上記の通り「SELinux re-labeling bind mount option is ignored on platforms without SELinux」と公式に明記されている。そのため `:Z` を付けたまま Mac 上で動かしても実害はなく、Linux（SELinux 有効環境）向けの互換性のために残しておいても問題ない。

## バインドマウントの他のオプション（`:Z`/`:z` 以外）

`:Z`/`:z` は SELinux ラベル専用のオプションだが、Docker のバインドマウント・ボリュームマウントには他にも用途別のオプションが存在する。

### アクセスモード

- `ro` / `rw`: 読み取り専用 / 読み書き可能。デフォルトは `rw`。

### bind propagation（マウント伝播設定）

ホスト側のマウントポイント配下にさらに別のマウントを重ねたとき、それをコンテナ側との間でどう伝播させるかの設定。`--mount`/`-v` どちらでも指定できる。

| オプション | 説明 | デフォルト |
|---|---|---|
| `rprivate` | オリジナル・レプリカどちらの側のマウントポイントも、互いに一切伝播しない | ✓（既定値） |
| `private` | `rprivate` に近いが、ネストしたサブマウントは対象外 | - |
| `shared` | オリジナルのサブマウントがレプリカ側に、レプリカのサブマウントがオリジナル側に、双方向に伝播する | - |
| `rshared` | `shared` を、ネストしたマウントポイントまで再帰的に適用する | - |
| `slave` | `shared` に似るが一方向のみ（オリジナル→レプリカ） | - |
| `rslave` | `slave` を再帰的に適用する | - |

### `--mount` フラグ限定のオプション

- `bind-recursive`: ホスト側でさらに別デバイスがマウントされているサブマウント領域の扱い。`enabled`（既定値。カーネル v5.12 以降なら再帰的に read-only 化）/ `disabled`（サブマウントを含めない）/ `writable` / `readonly`（v5.12 以降が必須）から選べる。`-v`/`--volume` では指定不可。
- `volume-nocopy`: 名前付きボリューム用。デフォルトでは空のボリュームにイメージ側の既存データが自動コピーされるが、これを無効化する。Postgres の公式イメージのようにコンテナ自身が初期化処理を行う場合に使う。

### 現在は非推奨・公式ドキュメントから消えたオプション

- `consistent` / `cached` / `delegated`: macOS の Docker Desktop 向けに存在した性能チューニング用オプション（ホストとコンテナ間のファイル同期について、即時性と整合性のトレードオフを調整するもの）。VirtioFS などファイル共有バックエンドの改善に伴い、現在の公式ドキュメントには記載がなくなっている。

## 読み方の注意

- 一次情報は `docs.docker.com`（Docker Engine の bind mounts ページ、および Compose ファイルリファレンス）。
- Red Hat Developer の記事は、SELinux 自体のメンテナ組織である Red Hat による技術解説であり、公式ドキュメントの記述を補強する一次情報に近い性質を持つ。`z`/`Z` の内部的な意味（共有ラベル・プライベートラベルの相互作用）は公式ドキュメントより詳しいため、この記事側の記述を優先して引用した。
- Docker Community Forums の投稿（`services` でラベルが無視される件）は、内容が Compose 公式リファレンスの記述と一致することを確認済みのため、Sources には一次ソースである公式リファレンスのみを掲載する。
- `consistent`/`cached`/`delegated` は現在の公式ドキュメントに記載が見当たらず、Web検索結果（ブログ・GitHub Issue等の二次情報）からの再構成である。「非推奨・記載が消えている」という事実そのものの一次確認はできていない点に注意。

## Sources

- [Bind mounts — Docker Docs](https://docs.docker.com/engine/storage/bind-mounts/)
- [Compose file reference (services) — Docker Docs](https://docs.docker.com/reference/compose-file/services/)
- [Volumes — Docker Docs](https://docs.docker.com/engine/storage/volumes/)
- [My advice on SELinux container labeling — Red Hat Developer](https://developers.redhat.com/articles/2025/04/11/my-advice-selinux-container-labeling)
