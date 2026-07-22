# Requirements Document

## Project Description (Input)
ダークモードの実装と切り替え機能(+システム連動)

関連Issue: #58

## 現状調査 (2026-07-22)

Web UI の実体は `static/` 配下である。`src/web/mod.rs` のルータは
`fallback_service(ServeDir::new("static"))` を使い、ログイン画面は
`src/web/routes.rs` が `static/login.html` を読み込んでいる。

- `static/index.html` … 実際に配信されるダッシュボード。Tailwind + DaisyUI 構成で
  `<html data-theme="dark">` を直書きしており、ダーク固定。
- `static/login.html` … 同上でダーク固定。
- `src/web/templates/index.html` … Ionic ベースの旧UI。どのルートからも参照されておらず、
  参照している `ionic-theme-dark-system.css` は CDN 上で 404 になる。今回の対象外とする。

## Requirements

### R1: テーマ選択

- **E (Event)**: 利用者がテーマ切替UIで「自動」「ライト」「ダーク」のいずれかを選んだとき
- **A (Actor)**: 利用者 / Web UI のフロントエンドスクリプト
- **R (Response)**: `<html>` の `data-theme` 属性を `light` / `dark` に更新し、
  配色を即座に切り替える。「自動」の場合は `prefers-color-scheme` の結果に従う
- **S (System)**: `static/index.html`

### R2: 選択の永続化

- **E**: テーマが選択されたとき
- **A**: フロントエンドスクリプト
- **R**: 選択値 (`auto` / `light` / `dark`) を `localStorage` に保存し、
  次回以降のページ読込時に復元する。`localStorage` が使えない環境でも例外で停止しない
- **S**: `static/index.html`, `static/login.html`

### R3: ちらつき防止

- **E**: ページが読み込まれたとき
- **A**: `<head>` 内のインラインスクリプト
- **R**: 描画前に `data-theme` を適用し、既定テーマからの切り替わり (FOUC) を発生させない
- **S**: `static/index.html`, `static/login.html`

### R4: システム設定への追従

- **E**: OS のカラースキーム設定が変更されたとき
- **A**: `matchMedia('(prefers-color-scheme: dark)')` のリスナ
- **R**: 選択が「自動」の場合のみ、リロードなしで表示テーマを切り替える
- **S**: `static/index.html`

### R5: ライトテーマの可読性

- **E**: ライトテーマが適用されているとき
- **A**: Web UI
- **R**: 背景・パネル・フォーム・ログ表示などの前景色と背景色が反転し、
  文字が判読可能なコントラストを保つ
- **S**: `static/index.html`, `static/login.html`

### R6: 非対象

- 既存の Rust 側 (`src/`) の振る舞いは変更しない。テーマはクライアント側のみで完結する

## 完了条件

- 3状態の切替UIがダッシュボードのヘッダに存在する
- リロード後も選択が保たれる
- 「自動」でOS設定に追従する
- `cargo fmt` / `cargo clippy --all-targets -- -D warnings` / `cargo test` が通る
