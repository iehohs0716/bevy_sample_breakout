# DynamoDB用スキーマを設計文書として持つ：JSON Schemaの採用と description の認識齟齬

日付: 2026-07-30

DynamoDBはPostgresのDDLやOpenAPIのような型付きスキーマ強制を持たない。それでも
「設計文書としてのスキーマ定義」を用意したいというユーザー要望に対し、JSON Schemaで
別ファイルを作成した際の経緯と、その過程で起きた `description` を巡る認識齟齬の記録。

関連: [[20260730_game-scenario-data-model-supabase-dynamodb]]

---

## 1. 質問：DynamoDBにOpenAPIのような定義書はあるか

ユーザーからの質問: 「DynamoDB用に、OpenAPIの定義書のような、スキーマを定義しておくものは
あるのか？」

回答の要点:

- DynamoDB自体には、Postgresの DDL や OpenAPI のような**型付きスキーマ強制の仕組みは無い**。
  強制されるのはパーティションキー／ソートキーの型と GSI／LSI 構成のみで、**アイテム本体は
  完全にスキーマレス**。
- ただし「OpenAPIっぽい役割」を担うエコシステムのツールは存在する。
  - **ElectroDB** / **DynamoDB Toolbox**（いずれも TypeScript製。エンティティ単位で属性の型や
    キー構成を宣言的に書ける、single-table設計向けのライブラリ）
  - 汎用の **Zod** / **JSON Schema** をバリデーション層として使う方法

---

## 2. 要望の具体化：実装コードではなく設計文書として、backend.mdとは別ファイルで

ユーザーの要望は「実装コードではなく設計文書として、しかも `backend.md` に直接書くのではなく
別ファイルで用意してほしい」というものだった。

これを受けて `doc_arch/schemas/game-scenario.schema.json` を JSON Schema
（`$schema: https://json-schema.org/draft/2020-12/schema`）として新規作成した。構造:

- ルートに `GameScenario` スキーマ（`scenarioId` をパーティションキーとして明記）。
- `$defs` で以下に分解:
  - `scenarioParameters`（`games` 配列。順序がプレイ順を表す）
  - `game`
  - `imageRef`（Supabase Storage のURL参照。バイト列は埋め込まない旨を `description` に明記）
  - `cellSize`
  - `brickPlacement`
  - `point`
  - `stats`

`backend.md` §5.1 からこのファイルへリンクし、「DynamoDB自体はスキーマレスだが、アイテムの
契約はこの JSON Schema を正とする」という位置づけを明記した。

---

## 3. 認識齟齬：「description が欲しい」の意味を取り違えた

この後、ユーザーから「シナリオとかゲームの説明も欲しい」という依頼があった。

### 3.1 最初の（誤った）対応

最初は、**JSON Schema の `description` キーワード**（スキーマ自体・プロパティ自体に対する
注釈・メタ情報）として、シナリオ／ゲームの概念説明を追記する対応をした。

### 3.2 実際の意図

しかしユーザーの意図は全く違い、「`title` と並ぶ**実データのプロパティ**として `description`
という**フィールド**を追加してほしい（ゲーム作者が書く説明文のような、実際にDynamoDBに
保存されるデータ項目）」という意味だった。ユーザーからは「そういうよく分からないやつは
要らない」と明確に修正が入った。

### 3.3 一般化した教訓

`description` という単語は、少なくとも以下の**2通りに解釈できる同綴異義**であり、
要求時にどちらを指しているか曖昧になりやすい。

| 解釈 | 意味 | 例 |
|---|---|---|
| (a) メタ情報としての description | スキーマ言語（JSON Schema等）のキーワードそのもの。仕様書を読む人向けの注釈 | `"description": "Partition key. ..."` |
| (b) データフィールドとしての description | モデリング対象のエンティティが実際に持つ属性 | ゲーム作者が入力する紹介文 |

相手が「◯◯の説明が欲しい」と言った時、**スキーマ設計の文脈では特に**この2つを区別せず
早合点しやすい。曖昧なら確認するか、少なくとも両方の可能性を意識してから着手すべきだった。

### 3.4 副次的なミス：頼まれていない title の追加

`description` フィールドを `game` オブジェクトに追加する作業の中で、頼まれていない
`title` フィールドまで誤って一緒に追加してしまうミスもあった。これはユーザーに指摘される前に
自分で気づいて削除した。

> UMLに存在しない属性を勝手に追加しない、という基本方針の再確認。「ついでに足しておくと
> 便利そう」という判断は、たとえ善意でも設計上の合意を経ていない変更であり、避けるべき。

### 3.5 最終形

`game-scenario.schema.json` のルート（`GameScenario`）と `$defs.game` の両方に、`title` と
同様の必須文字列フィールドとして `description` を追加した（`required` にも追加）。

```json
"required": ["scenarioId", "title", "description", "authorId", "visibility", "createdAt", "scenarioParameters", "stats"],
...
"description": { "type": "string" }
```

（`game` エンティティ側も同様に `description` を必須プロパティとして追加。）

---

## 4. 教訓（一般化）

1. DynamoDBのようなスキーマレスなKVSでも、「設計文書としての契約」を明文化したい要望には
   JSON Schema / Zod 等の宣言的スキーマ言語で応えられる。実装ライブラリ（ElectroDB等）の
   型定義と、設計文書としてのスキーマファイルは別物であり、要望がどちらかを確認する。
2. **"description" のような、スキーマのメタ情報とデータフィールドの両方に使われがちな単語は
   要求時に誤読しやすい。** 「◯◯の説明が欲しい」と言われたら、それが (a) ドキュメント上の
   注釈なのか (b) 実データの属性なのかを、可能なら先に確認する。
3. 修正作業のついでに、頼まれていない属性（今回は `title`）を「あると自然だから」と
   独断で追加しない。UML等の合意済みモデルに存在しない属性は追加しない。
