# Blog AutoPost CLI (Rust Port) 移行計画書

> **状態 (2026-07-22)**: Rust移植は完了し、`main` は Rust版のみで構成されている。本書は移植当時の計画を残した履歴文書であり、現行仕様の参照先は `README.md` と `CLAUDE.md` とする。


## 目的
既存のPython版（FastAPI等ベース）の挙動を再現しつつ、Rustに移行することで、メモリ要件の削減（KISS原則に基づく軽量化）とx86_64 Linuxへのクロスコンパイル対応（完全静的リンクバイナリの生成）を実現する。

## 開発原則
1. **TDD (テスト駆動開発)**: ロジックを追加する前にテストを記述し、こまめにコミットを行う。
2. **KISS (Keep It Simple, Stupid)**: 余計な機能や過剰な抽象化を避け、シンプルな設計を心がける。
3. **プラグインの静的組み込み**: Python版の動的読み込みアーキテクチャは廃止し、コンパイル時にすべての機能を組み込み、設定（`config.yml`）でON/OFFを切り替える。
4. **クロスコンパイル対応**: C言語ライブラリへの依存を排除（`reqwest` + `rustls`、SQLiteは `sqlx` を使用）。`x86_64-unknown-linux-musl` ターゲットへのコンパイルを前提とする。

## 開発マイルストーン

### Phase 1: プロジェクトの初期化と設定・基盤作成
- `cargo init` によるRustプロジェクトのセットアップ
- 依存関係の定義 (`serde`, `tokio`, `reqwest`, `sqlx` など)
- `config.yml` のパース処理とデータ構造定義 (TDDで実装)

### Phase 2: コアロジックの実装
- **RSSパーサー**: `feed-rs` を用いた記事フェッチ処理
- **テキスト処理**: SNS毎の文字数チェック・切り詰め、およびURL短縮（is.gd）の実装
- **メディア処理**: `image` クレートを用いた画像リサイズ処理

### Phase 3: SNSクライアント実装 (順次TDD)
各SNSごとにHTTPクライアント (`reqwest`) を用いて投稿処理を実装する。
1. Bluesky
2. Mastodon
3. Misskey
4. X (Twitter)
5. Threads

### Phase 4: データベースとスケジューリング
- `sqlx` + SQLite を用いた予約データの永続化
- `tokio-cron-scheduler` を用いた定期実行（フィード監視・予約投稿実行）

### Phase 5: Webサーバーの実装
- `axum` + `askama` を用いたWeb UIの実装
- 手動投稿・予約投稿のエンドポイント作成

### Phase 6: クロスコンパイルと最終調整
- Mac M4 (ARM64) から x86_64 Linux (musl) へのクロスコンパイル環境の構築（`cross` 等を使用）
- CI/CD または Makefile / justfile の更新
- 最終テスト
