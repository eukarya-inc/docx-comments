# docx-comment-json

`.docx` ファイルから Word コメントを抽出し、AI が後段で処理しやすい JSON を標準出力に出す Rust 製 CLI です。

## できること

- コメント本文の抽出
- author / created_at の抽出
- `document.xml` から対象箇所の抽出
- text / image / table の対象種別の判定
- 画像コメントの caption / 画像メタデータの補助抽出
- テーブルコメントの table index / caption の補助抽出
- `commentsExtended` 系パートから返信関係と done 状態の復元
- 欠損パートや壊れた XML を diagnostics として報告

## 使い方

### 開発中に実行

```bash
cargo run -- sample.docx
```

### ビルドして実行

```bash
cargo build --release
./target/release/docx-comment-json sample.docx
```

### pretty JSON

```bash
docx-comment-json sample.docx --pretty
```

### JSON ファイルに書き込む

```bash
docx-comment-json sample.docx --pretty > comments.json
```

`stdout` に JSON が出るので、`>` でそのままファイル保存できます。

### extended part が無い場合に失敗させる

```bash
docx-comment-json sample.docx --fail-on-missing-extended=true
```

### raw_text を含めない

```bash
docx-comment-json sample.docx --include-raw=false
```

## CLI 仕様

```text
docx-comment-json <input.docx>
docx-comment-json <input.docx> --pretty
docx-comment-json <input.docx> --fail-on-missing-extended=true
docx-comment-json <input.docx> --include-raw=false
```

- JSON は `stdout` に出ます
- エラーは `stderr` に出ます
- 失敗時は非 0 終了コードです

## 出力 JSON の形

トップレベルは次の形です。

```json
{
  "schema_version": "1.0",
  "source": {
    "file": "sample.docx"
  },
  "comments": [
    {
      "comment_id": "5",
      "author": "Alice",
      "created_at": "2024-01-01T10:00:00Z",
      "done": false,
      "anchor": {
        "target_type": "text",
        "raw_text": "導入の説明",
        "locator": {
          "heading_path": ["1. 背景", "1.1 調査目的"],
          "before_context": "背景として、",
          "after_context": "について説明する。",
          "table_cell": null
        },
        "context": {
          "caption": null
        },
        "image": null,
        "table": null,
        "start_hint": null,
        "end_hint": null
      },
      "comment": {
        "raw_text": "根拠を追加してください"
      },
      "replies": [
        {
          "comment_id": "6",
          "author": "Bob",
          "created_at": null,
          "done": null,
          "comment": {
            "raw_text": "明日対応します"
          },
          "metadata": {
            "parent_comment_id": "5",
            "comment_para_id": "p2",
            "has_extended_properties": true,
            "source_parts": [
              "word/comments.xml",
              "word/commentsExtended.xml"
            ]
          }
        }
      ],
      "metadata": {
        "parent_comment_id": null,
        "comment_para_id": "p1",
        "has_extended_properties": true,
        "source_parts": [
          "word/comments.xml",
          "word/commentsExtended.xml",
          "word/document.xml"
        ]
      }
    }
  ],
  "diagnostics": {
    "warnings": [],
    "missing_parts": []
  }
}
```

## JSON プロパティの意味

### top level

- `schema_version`
  - 出力 JSON のスキーマバージョン
- `source.file`
  - 入力した `.docx` ファイルパス
- `comments`
  - ルートコメントの配列
- `diagnostics`
  - 欠損パートや推定・破損に関する補助情報

### comment / reply

- `comment_id`
  - Word コメント ID
- `author`
  - コメント作成者名
- `created_at`
  - コメント作成日時
- `done`
  - 対応済み状態
  - `true` / `false` / `null`
- `comment.raw_text`
  - XML から読んだ元の本文
- `replies`
  - そのコメント配下の返信配列

### anchor

- `anchor.target_type`
  - 対象が `text`, `image`, `table` のどれか
- `anchor.target_type = text`
  - 通常の本文コメント
- `anchor.target_type = image`
  - 画像コメント
  - `anchor.image.*` が入ることがあります
  - 近傍 caption を推定できた場合は `anchor.context.caption` に入ります
- `anchor.target_type = table`
  - テーブルコメント
  - `anchor.table.index` が入ります
  - 近傍 caption を推定できた場合は `anchor.context.caption` に入ります
- `anchor.raw_text`
  - 対象箇所の元テキスト
  - 画像コメントでは `null` のことがあります
- `anchor.locator`
  - 修正対象を見つけやすくするための構造的な位置情報
- `anchor.locator.heading_path`
  - 対象箇所が属する見出し階層
- `anchor.locator.before_context`
  - 対象文字列の直前コンテキスト
- `anchor.locator.after_context`
  - 対象文字列の直後コンテキスト
- `anchor.locator.table_cell`
  - table コメント時のセル位置
- `anchor.locator.table_cell.row_index`
  - table 内の行番号
- `anchor.locator.table_cell.column_index`
  - table 内の列番号
- `anchor.context.caption`
  - 近傍 paragraph などから推定した caption
- `anchor.image`
  - 画像コメントの補助情報
- `anchor.image.name`
  - Word 内の画像名
- `anchor.image.alt_text`
  - 画像の alt text / description
- `anchor.image.relationship_id`
  - 画像リソースへの relationship ID
- `anchor.table`
  - テーブルコメントの補助情報
- `anchor.table.index`
  - 文書中で何番目の table か
- `anchor.start_hint`
  - 将来の位置ヒント用
- `anchor.end_hint`
  - 将来の位置ヒント用

### metadata

- `metadata.parent_comment_id`
  - 返信の直親 comment_id
  - ルートコメントでは `null`
- `metadata.comment_para_id`
  - コメント本文側 paragraph の paraId
- `metadata.has_extended_properties`
  - extended 系パート由来の追加情報があるか
- `metadata.source_parts`
  - その項目の構築に使った Word パート一覧

### diagnostics

- `diagnostics.missing_parts`
  - `.docx` 内に存在しなかった必要パート
- `diagnostics.warnings`
  - 処理継続はできたが注意が必要な項目
- `diagnostics.warnings[].code`
  - 警告種別
- `diagnostics.warnings[].message`
  - 警告内容
- `diagnostics.warnings[].comment_id`
  - 関連 comment_id
- `diagnostics.warnings[].source_part`
  - 警告の発生元パート

## 通常運用での前提

普通の Word コメント運用であれば実用上問題ない想定です。特に次のケースをカバーしています。

- 通常のコメント本文
- 返信付きコメント
- done 状態付きコメント
- run をまたぐ対象箇所
- 画像コメント
- テーブルコメント
- comment range が無い場合の段落 fallback
- コメント本文が表の中にあるケース

一方で、かなり特殊な `.docx` 構造や標準外のパート配置までは保証していません。

## テスト

```bash
cargo test
```
