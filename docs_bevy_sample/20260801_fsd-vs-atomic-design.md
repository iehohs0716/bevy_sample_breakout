# FSDとAtomic Designの比較

日付: 2026-08-01

## 1. 位置づけ

`docs_bevy_sample/20260801_fsd-overview.md`で整理したFSD（Feature-Sliced Design）を、
UIコンポーネント分類手法として広く知られるAtomic Designと比較して解説する。

関連ドキュメント: `docs_bevy_sample/20260801_fsd-overview.md`（レイヤー・スライス・
セグメントの全体像）、`docs_bevy_sample/20260801_fsd-dependency-rules.md`（依存方向の
詳細）。

## 2. Atomic Designの分類軸

atoms（最小のUI部品）→molecules（atomsの組み合わせ）→organisms（複雑なUIブロック）
→templates→pagesという、**純粋にUIの複雑さ・入れ子の深さだけ**で分類する手法。
ビジネス上の意味（ドメイン）は分類に一切関与しない。

## 3. FSDとの違い1: ドメインで分けるか見た目の複雑さで分けるか

Atomic Designでは、汎用的な`Button`（ドメインに依存しない部品）と、特定ドメインに依存する
`LevelCard`のような部品が、見た目の複雑さが近ければ同じ"molecule"や"organism"という分類に
一緒くたに入ってしまう。

FSDは`entities/level`のように最初からドメイン名でスライスを切るため、「これはLevelという
ドメインの部品だ」という情報がフォルダ構造そのものに表れる。

## 4. FSDとの違い2: データ・ロジックの置き場の有無

Atomic Designは見た目（UI）だけの分類法であり、「APIをどこで呼ぶか」「型定義をどこに
置くか」には答えを持たない。そのため実プロジェクトでは、atoms/molecules/organismsとは
別に`hooks/`や`store/`や`api/`のような並行構造を追加する必要が生じがちである。

FSDは各スライスの中に`ui`(見た目)・`model`(型)・`api`(データ取得)・`lib`(ロジック)という
セグメントが最初から用意されており、「あるドメインに関するもの」が1箇所に揃う。本リポジトリ
でも`entities/level`の中に型・データ・レイアウト計算ロジック・見た目のコンポーネントが
全て同居している。

## 5. FSDとの違い3: 依存方向が強制される

FSDは「下のレイヤーは上のレイヤーをimportしてはいけない」というルールを持ち、lintツール
（`steiger`等）で機械的に強制できる。Atomic Designにはこの種の強制力がなく、organism同士が
互いをimportし合ったりatomがビジネスロジックを持ってしまったりすることを方法論として
止める仕組みがないため、プロジェクトが育つにつれて依存関係が乱雑化しやすいという弱点がある。

## 6. 補足: 両者は排他的ではない

FSDの`shared/ui`（汎用UIキット）レイヤーの中では、Atomic Design的な「Button、Inputの
ような純粋な見た目部品の粒度分け」という考え方がそのまま活きる。「汎用UI部品の粒度」は
Atomic Design、「ドメイン境界と依存方向」はFSD、と役割分担して併用されることが実務では
多い。
