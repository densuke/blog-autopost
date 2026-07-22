# Requirements Document

## Project Description (Input)
予約投稿状態のメッセージ編集にて、SNSを全て選び直せるようにする(現在はなぜか2つしか出てこないバグ)

## 背景と調査結果 (2026-07-22)

Issue #56 / #57 の調査を行った。

- 実際に配信されている Web UI は `static/index.html` である。
  `src/web/mod.rs` の `ServeDir::new("static")` が唯一の配信経路であり、
  `src/web/templates/` 配下は Python 実装時代の Jinja テンプレートの残骸で、
  Rust 実装からは一切参照されていない。
- Issue 起票元の spec は 2025-10-18 作成であり、Jinja 版 UI 時代のものである。
  Jinja 版は `{% for account in sns_accounts %}` を含み、テンプレートエンジンなしで
  そのまま配信すると「全てのSNS」と未展開の1行の計2件しか表示されない。
  これが「2つしか出てこない」の症状と一致する。
- 現行の `static/index.html` では、ヘッドレスブラウザで検証した結果、
  設定した4件の SNS が編集モーダルにも全て表示された。よって
  「2件しか出ない」現象そのものは現行 UI では再現しない。
- ただし現行 UI には、SNS 選択に関する実際の欠陥が2点残っている。
  1. 編集モーダルの選択肢を `/api/config` ではなく、
     手動投稿フォームの DOM (`input[name="sns-target"]`) から作っている。
     設定取得に失敗した場合や手動投稿フォームの構造が変わった場合、
     編集モーダルの選択肢が0件になり、SNS を選び直せなくなる。
  2. チェックボックスの `value` に表示用文字列「種別 (名前)」を入れ、
     保存時に正規表現でアカウント名を取り出している。
     アカウント名自体に括弧が含まれる場合 (例: `my(test)`) に
     名前を復元できず、既存の選択状態が復元されない、
     または誤った名前で保存される。

本 spec では上記2点を根本原因として扱い、
「SNS アカウント一覧はサーバを唯一の情報源として構造化して返し、
UI は表示用ラベルと実名を分離して扱う」形へ修正する。

## Requirements

### REQ-1: SNS アカウント一覧の構造化提供

**EARS**

- **Event**: Web UI が `/api/config` を要求したとき
- **Actor**: Web サーバ (`get_config`)
- **Response**: `config.yml` に定義された全ての既知種別 SNS について、
  アカウント名 (`name`)、種別 (`sns_type`)、表示用ラベル (`label`) を持つ
  `sns_accounts` 配列を返す。未知種別 (`Unknown`) は含めない。
- **System**: `src/web/routes.rs`

**受け入れ条件**

- 設定に4件の SNS があるとき、`sns_accounts` は4要素を返すこと。
- `label` は「X (name)」形式であること。
- アカウント名に括弧が含まれていても `name` は元の値のまま返すこと。
- 既存クライアント互換のため `active_sns` (ラベル文字列の配列) も返し続けること。

### REQ-2: 投稿対象 SNS の指定を名前でも受け付ける

**EARS**

- **Event**: Web UI が `/api/post` に投稿対象 SNS を指定して送信したとき
- **Actor**: Web サーバ (`manual_post`)
- **Response**: 指定文字列がアカウント名と表示用ラベルのいずれであっても
  対応する SNS を一意に解決する。解決できない場合のみエラーとする。
- **System**: `src/web/routes.rs`

**受け入れ条件**

- `x` でも `X (x)` でも同じ SNS が解決されること。
- アカウント名に括弧を含む場合でも名前指定で解決できること。
- 存在しない指定は解決されず、既存どおりエラー結果を返すこと。

### REQ-3: 編集モーダルの選択肢はサーバ由来の一覧から生成する

**EARS**

- **Event**: 利用者が予約投稿の編集ボタンを押したとき
- **Actor**: Web UI (`static/index.html` の `editPost`)
- **Response**: `/api/config` から取得済みの `sns_accounts` を唯一の情報源として
  全ての SNS のチェックボックスを生成する。
  手動投稿フォームの DOM には依存しない。
- **System**: `static/index.html`

**受け入れ条件**

- 設定された SNS の件数と、編集モーダルに並ぶチェックボックスの件数が一致すること。
- 各チェックボックスの `value` はアカウント名そのものであること。
- 予約投稿の `target_sns` に含まれる SNS だけが選択済みになること。

### REQ-4: 保存時の SNS 名を文字列解析なしで送る

**EARS**

- **Event**: 利用者が編集モーダルで保存したとき
- **Actor**: Web UI (`saveEdit`)
- **Response**: チェックされたチェックボックスの `value` (アカウント名) を
  そのまま `target_sns` として送信する。正規表現による抽出は行わない。
- **System**: `static/index.html`

**受け入れ条件**

- アカウント名に括弧が含まれていても、保存後の `target_sns` が元の名前と一致すること。

### REQ-5: 死んだテンプレートによる誤解の解消

**EARS**

- **Event**: 開発者が Web UI を修正しようとしたとき
- **Actor**: 開発者
- **Response**: 参照されていない `src/web/templates/` が
  現行 UI であると誤認しないよう、状況をドキュメント化する。
- **System**: 本 requirements.md

**受け入れ条件**

- 現行の配信対象が `static/` であることが明記されていること。

## 完了の定義

- REQ-1、REQ-2 に対応する Rust 側ユニットテストが存在し通ること。
- REQ-3、REQ-4 はフロントエンドのみの修正であり、
  ヘッドレスブラウザによる手動検証で確認する。
- `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test` が通ること。
- 編集モーダルで設定済みの全 SNS を選び直せること。

---

## 実装完了 (2026-07-22)

PR #69 で修正済み。関連Issue #57 はクローズ済み。

### 原因

編集モーダルのSNSチェックボックスを、手動投稿フォーム側のDOMを走査して
生成していた。手動投稿フォームの表示状態によって拾える項目が変わるため、
選択肢が一部しか出ない状態になっていた。

### 修正内容

`/api/config` が返す `sns_accounts` を唯一の情報源とし、そこから
編集モーダルのチェックボックスを生成するようにした。DOMへの依存が
無くなったため、設定済みのSNSが常にすべて表示される。

`static/index.html` の該当箇所:

```javascript
// 設定済みの全SNSを /api/config 由来の一覧から生成する
// (手動投稿フォームの DOM には依存しない)
snsAccounts.forEach(account => { ... });
```

あわせてサーバ側に `sns_accounts`(name / sns_type / label の構造化一覧)が
追加され、旧形式のラベル文字列 `active_sns` も互換のため維持されている。

