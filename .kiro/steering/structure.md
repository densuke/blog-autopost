# プロジェクト構造 (structure.md)

## 1. ルートディレクトリ構成

プロジェクトのルートディレクトリは、アプリケーションのソースコード、設定ファイル、ドキュメント、テストコードなどを明確に分離する構造になっています。

```
/
├── .env.example         # 環境変数設定のテンプレート
├── .gitignore           # Gitの追跡対象外ファイルを指定
├── .kiro/               # Kiro(AI開発ツール)のステアリングファイル
│   └── steering/
├── .python-version      # 使用するPythonのバージョンを指定
├── CLAUDE.md            # Claude AI用の設定・指示ファイル
├── config.yml.template  # アプリケーション設定ファイルのテンプレート
├── justfile             # justコマンドランナーの定義ファイル
├── pyproject.toml       # プロジェクト定義と依存関係管理 (PEP 621)
├── pytest.ini           # pytestの設定ファイル
├── README.md            # プロジェクトの主要なドキュメント
├── uv.lock              # uvによってロックされた依存関係のバージョン
├── src/                 # アプリケーションのソースコード
├── tests/               # テストコード
├── docs/                # プロジェクト関連ドキュメント
└── data/                # 処理済み記事の記録など、アプリケーションが生成するデータ
```

## 2. サブディレクトリ構造

### `src/` - ソースコード

CLIワークフローとWeb UIが共有する主要ロジックがまとまっています。

- `main.py`: CLIのエントリーポイント。フィード監視、直接投稿、メンテナンス系コマンドを引数に応じて実行します。
- `article_manager.py`: RSS/Atomフィードの取得、新着記事の検出、処理済み記事の永続化を担当します。
- `config_manager.py`: `config.yml`の読み込みと設定管理。SNS認証情報の環境変数上書きやWebサーバー／認証設定の提供も行います。
- `plugin_loader.py`: `src/plugins`にあるSNS向けプラグインを動的に読み込み、CLIとWebの投稿処理で共有します。
- `image_resizer.py` / `media_converter.py` / `media_validator.py`: SNSごとの制限に合わせたメディアの変換・リサイズ・バリデーションを担います。
- `text_optimizer.py` / `url_shortener.py`: 文字数制限に応じたテキスト最適化とURL短縮処理を提供します。

### `src/plugins/` - SNSプラグイン

各SNSへの投稿機能を実装したプラグインが配置されています。この構造により、新しいSNSへの対応が容易になります。

- `__init__.py`: プラグインの基底クラス`SocialMediaPlugin`を定義します。
- `x.py`: X (旧Twitter) への投稿ロジック。
- `bluesky.py`: Blueskyへの投稿ロジック（リンクカード生成機能を含む）。
- `threads.py`: Threadsへの投稿ロジック。
- `mastodon.py`: Mastodonへの投稿ロジック。
- `misskey.py`: Misskeyへの投稿ロジック。
- `tumblr.py`: Tumblrへの投稿ロジック。

### `src/web/` - Web UIと予約投稿サービス

FastAPIベースのダッシュボードとバックグラウンドスケジューラを実装しています。

- `main_web.py`: FastAPIアプリ本体。ログイン、テンプレート描画、予約投稿API、APSchedulerの起動をまとめています。
- `runner.py`: `uvicorn`で`src.web.main_web:app`を起動するためのエントリーポイント。
- `posting_service.py`: Web経由の即時投稿をハンドリングし、メディアのリサイズ／バリデーションを共有キャッシュで最適化します。
- `core_posting_logic.py`: 既存CLIの投稿処理をクラス化したラッパー。Web UIやスケジューラからSNS投稿を再利用できるようにします。
- `post_executor.py`: 予約投稿を取り出してSNSへ送信し、結果に応じてステータスやエラーメッセージを更新します。
- `scheduler_service.py`: APSchedulerのバックグラウンドジョブを構成し、一定間隔で予約投稿を監視・実行します。
- `scheduled_post_model.py`: 予約投稿のデータクラス。JSON/DBとの相互変換やタイムゾーン正規化を担います。
- `scheduled_post_store.py`: JSONファイルを利用したストア実装（既存データとの互換用）。
- `scheduled_post_store_sqlite.py`: SQLAlchemy/SQLiteベースのストア実装。DAO (`dao.py`) とORMモデル (`models.py`) を通じて永続化・フィルタリングを行います。
- `auth_service.py`: 設定ファイルに定義された認証情報でログインを検証します。
- `timezone_utils.py`: ローカルタイムゾーンの正規化ユーティリティ。
- `templates/`: FastAPI用のJinja2テンプレート（`index.html`, `login.html`など）。

### `tests/` - テストコード

`pytest`を使用したユニットテストや結合テストが格納されています。ファイル名は`test_*.py`の形式で、テスト対象のモジュールと対応しています。

### `docs/` - ドキュメント

`README.md`だけでは収まらない詳細なセットアップ手順やAPIの仕様などを記述します。

### `data/` - データファイル

アプリケーションが実行時に生成・参照するデータが保存されます。処理済み記事を記録するJSON、予約投稿を保存する`scheduled_posts.db`（SQLite）、アップロードメディア用の`scheduled_media/`ディレクトリ、アプリケーションログなどがここに配置されます。

## 3. コード構成パターン

- **設定駆動開発**: アプリケーションの挙動の多くは`config.yml`によって制御されます。CLIだけでなくWebサーバーのホスト/ポートや認証情報もここに集約されています。
- **プラグインアーキテクチャ**: SNSへの投稿機能は疎結合なプラグインとして実装されています。`SocialMediaPlugin`基底クラスを継承し、`post`メソッドを実装することで、コアロジックに手を加えることなく新しいSNSに対応できます。
- **共有投稿パイプライン**: CLIとWeb UIは`core_posting_logic.py`および`posting_service.py`を通じて同じ投稿処理を利用し、単一のプラグイン群で両方のユースケースを賄います。
- **責任の分離**: 設定管理、記事管理、投稿実行、Web/DBアクセスなどをモジュール単位で分離しています。DAO層（`dao.py`）とモデル（`models.py`）により永続化処理も分離されています。
- **バックグラウンドスケジューリング**: APSchedulerを使った監視ジョブとSQLiteストレージを組み合わせて予約投稿を管理し、完了済み投稿のクリーンアップも自動化しています。

## 4. ファイル命名規則

- **Pythonモジュール**: スネークケース（例: `config_manager.py`）。
- **テストファイル**: `test_`プレフィックスを付けたスネークケース（例: `test_main.py`）。
- **クラス名**: アッパーキャメルケース（例: `ArticleManager`）。

## 5. インポート構成

- `src`ディレクトリをルートとする絶対インポートを基本とします。
  - 例: `from .config_manager import ConfigManager`

## 6. 主要な設計原則

- **拡張性**: 新しいSNSやブログフィード形式、URL短縮サービスなどを容易に追加できるよう、各機能はモジュール化・プラグイン化されています。
- **後方互換性**: 設定ファイルのフォーマット変更時（例: 単一アカウントから複数アカウント対応へ）にも、古い形式をサポートし、既存ユーザーが設定変更なしでアップデートできるよう配慮されています。
- **堅牢性**: メディア投稿前のバリデーションや、APIエラー発生時のフォールバック処理など、予期せぬエラーが発生してもアプリケーション全体が停止しないように設計されています。
- **ユーザーフレンドリーなCLI**: `--dry-run`や`--debug`、`--list-sns`といったオプションを提供し、ユーザーが動作を安全に確認・デバッグできるようにしています。
