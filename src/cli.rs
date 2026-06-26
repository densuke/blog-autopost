use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// 設定ファイルのパス
    #[arg(short, long, default_value = "config.yml")]
    pub config: String,

    /// 新着記事のチェック時に処理する記事数を制限
    #[arg(short, long)]
    pub limit: Option<usize>,

    /// 詳細なデバッグログを表示
    #[arg(long)]
    pub debug: bool,

    /// フィード取得などの詳細な診断情報を表示します
    #[arg(short, long)]
    pub verbose: bool,

    /// 添付メディアをセンシティブコンテンツとして扱います（現状 Misskey のみ対応）
    #[arg(long)]
    pub sensitive: bool,

    /// 登録されているSNSアカウントの一覧を表示します
    #[arg(long)]
    pub list_sns: bool,

    /// 登録されているフィードの一覧を表示します
    #[arg(long)]
    pub list_feeds: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// デーモンとしてスケジューラを起動し、定期実行する
    Run {
        /// ドライランモード（実際のSNSへの投稿とDB保存を行わない）
        #[arg(long)]
        dry_run: bool,
    },
    /// RSSフィードを一度だけチェックし、新着記事を各SNSへ投稿する
    Check {
        /// ドライランモード（実際のSNSへの投稿とDB保存を行わない）
        #[arg(long)]
        dry_run: bool,

        /// 投稿先のSNSを限定する（カンマ区切り。'-名前'で除外、'all'で全件）。省略時は全SNS
        #[arg(short, long)]
        sns: Option<String>,
    },
    /// 任意のテキストを指定したSNSへ手動投稿する
    Post {
        /// 投稿するテキスト
        #[arg(short, long)]
        text: String,

        /// 投稿先のSNS (例: 'mastodon', 'misskey')。省略または'all'指定時は全SNSが対象
        #[arg(short, long)]
        sns: Option<String>,

        /// インスタンスURL (引数で上書きする場合)
        #[arg(long, env = "SNS_URL")]
        instance_url: Option<String>,

        /// アクセストークン (引数で上書きする場合)
        #[arg(long, env = "SNS_TOKEN")]
        token: Option<String>,

        /// 添付するローカルの画像ファイルパス（複数指定可）
        #[arg(short, long)]
        media: Option<Vec<String>>,

        /// 添付するリンクURL
        #[arg(short, long)]
        link: Option<String>,
    },
    /// 現在のRSSフィードを取得し、すべて「既読（投稿済み）」として記録する
    Touch,
    /// Web UIを起動する
    Serve {
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
    /// 予約投稿を管理する（一覧、追加、削除、変更）
    Schedule {
        #[command(subcommand)]
        action: ScheduleAction,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ScheduleAction {
    /// 予約一覧を表示する
    List {
        /// 特定のステータスでフィルタリングする（例: '予約済み', '投稿済み', '失敗'）
        #[arg(short, long)]
        status: Option<String>,
    },
    /// 新しい予約投稿を追加する
    Add {
        /// 投稿するテキスト
        #[arg(short, long)]
        text: String,

        /// 投稿予定時刻 (RFC3339形式。例: '2026-06-20T15:00:00+09:00' もしくは 'YYYY-MM-DD HH:MM')
        #[arg(short, long)]
        at: Option<String>,

        /// 自動で空いている次の投稿枠を検索して設定する
        #[arg(long)]
        auto_slot: bool,

        /// 投稿先のSNS (例: 'mastodon', 'misskey')。カンマ区切りで複数指定可。省略時は全SNS
        #[arg(short, long)]
        sns: Option<String>,

        /// 添付するローカルの画像ファイルパス（複数指定可）
        #[arg(short, long)]
        media: Option<Vec<String>>,

        /// 添付するリンクURL
        #[arg(short, long)]
        link: Option<String>,
    },
    /// 予約投稿を削除する
    Delete {
        /// 削除する予約投稿 of ID
        id: String,
    },
    /// 予約投稿を変更する
    Update {
        /// 変更する予約投稿のID
        id: String,

        /// 変更後のテキスト
        #[arg(short, long)]
        text: Option<String>,

        /// 変更後の投稿予定時刻 (RFC3339形式)
        #[arg(short, long)]
        at: Option<String>,

        /// 変更後のSNS（カンマ区切り。例: 'mastodon,bluesky'）
        #[arg(short, long)]
        sns: Option<String>,

        /// 変更後のステータス（'予約済み', '投稿済み', '失敗'）
        #[arg(long)]
        status: Option<String>,

        /// 変更後のリンクURL
        #[arg(short, long)]
        link: Option<String>,
    },
}
