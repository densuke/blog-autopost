use blog_autopost_rs::config::{self, Config};
use blog_autopost_rs::{scheduled, timing};

use crate::cli::ScheduleAction;

/// 予約投稿の保存先(既定)
const DEFAULT_STORE_PATH: &str = "data/scheduled_posts.json";
/// 添付メディアの退避先ディレクトリ(既定)
const DEFAULT_UPLOADS_DIR: &str = "data/uploads";

/// `schedule` サブコマンド（一覧・追加・削除・変更）を処理する。
pub async fn run(action: ScheduleAction, config_data: &Config) -> anyhow::Result<()> {
    std::fs::create_dir_all("data").ok();
    run_with_paths(action, config_data, DEFAULT_STORE_PATH, DEFAULT_UPLOADS_DIR).await
}

/// 保存先を指定して `schedule` サブコマンドを処理する。
///
/// `run` は既定のパスを渡してこの関数を呼ぶ。テストからは一時ディレクトリを
/// 渡すことで、実際の `data/` を汚さずに検証できる。
async fn run_with_paths(
    action: ScheduleAction,
    config_data: &Config,
    store_path: &str,
    uploads_dir: &str,
) -> anyhow::Result<()> {
    let scheduled_store = scheduled::JsonScheduledPostStore::new(store_path);

    match action {
        ScheduleAction::List { status } => {
            let posts = scheduled_store.get_all_posts().await?;
            let mut filtered_posts = posts;
            if let Some(status_filter) = status {
                filtered_posts.retain(|p| p.status == status_filter);
            }

            // 予定時間順にソート
            filtered_posts.sort_by_key(|p| p.scheduled_at);

            println!(
                "{:<25} {:<20} {:<20} {:<10} Content Preview",
                "ID", "Scheduled At", "SNS", "Status"
            );
            println!("{}", "-".repeat(100));
            for post in filtered_posts {
                let content_preview = if post.content.chars().count() > 30 {
                    format!("{}...", post.content.chars().take(27).collect::<String>())
                } else {
                    post.content.clone()
                };
                let sns_str = post.target_sns.join(",");
                println!(
                    "{:<25} {:<20} {:<20} {:<10} {}",
                    post.id,
                    post.scheduled_at.format("%Y-%m-%d %H:%M:%S"),
                    sns_str,
                    post.status,
                    content_preview
                );
            }
        }
        ScheduleAction::Add {
            text,
            at,
            auto_slot,
            sns,
            media,
            link,
        } => {
            // SNS ターゲットの決定
            let mut target_sns = Vec::new();
            if let Some(sns_arg) = sns {
                for part in sns_arg.split(',') {
                    let part = part.trim();
                    if !part.is_empty() {
                        target_sns.push(part.to_string());
                    }
                }
            } else {
                // 省略時は config.yml 内の全 SNS を取得
                for sns_conf in &config_data.sns {
                    let name = match sns_conf {
                        config::SnsConfig::Mastodon { name, .. } => name,
                        config::SnsConfig::Misskey { name, .. } => name,
                        config::SnsConfig::Bluesky { name, .. } => name,
                        config::SnsConfig::X { name, .. } => name,
                        config::SnsConfig::Threads { name, .. } => name,
                        config::SnsConfig::Tumblr { name, .. } => name,
                        _ => continue,
                    };
                    target_sns.push(name.clone());
                }
            }

            if target_sns.is_empty() {
                println!("Error: No target SNS configured or specified.");
                return Ok(());
            }

            // 画像ファイルのコピー（退避）処理
            let mut processed_media = Vec::new();
            if let Some(ref media_list) = media {
                if let Err(e) = std::fs::create_dir_all(uploads_dir) {
                    println!("Error: Failed to create uploads directory: {:?}", e);
                    return Ok(());
                }
                for file_path in media_list {
                    let path = std::path::Path::new(file_path);
                    if !path.exists() {
                        println!("Error: Media file not found: {}", file_path);
                        return Ok(());
                    }

                    let file_name = path
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("image.png");

                    let sanitized_name: String = file_name
                        .chars()
                        .map(|c| {
                            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                                c
                            } else {
                                '_'
                            }
                        })
                        .collect();

                    let timestamp = chrono::Utc::now().timestamp_micros();
                    let unique_name = format!("{}_{}", timestamp, sanitized_name);
                    let save_path = format!("{}/{}", uploads_dir, unique_name);

                    if let Err(e) = std::fs::copy(file_path, &save_path) {
                        println!(
                            "Error: Failed to copy media file {} to {}: {:?}",
                            file_path, save_path, e
                        );
                        return Ok(());
                    }

                    processed_media.push(save_path);
                }
            }

            // 時刻の決定
            use chrono::TimeZone;
            let scheduled_time = if auto_slot {
                let timing_manager = timing::TimingManager::new(config_data);
                let finder = timing::SlotFinder::new(&timing_manager, &scheduled_store, 5);

                let mut created_posts = Vec::new();
                for sns_name in &target_sns {
                    match finder.find_next_available_slot(sns_name, None, 7).await {
                        Ok(Some(dt)) => {
                            let mut post = scheduled::ScheduledPost::new(
                                text.clone(),
                                dt,
                                processed_media.clone(),
                                vec![sns_name.clone()],
                            );
                            post.link_url = link.clone();
                            let created = scheduled_store.create_post(post).await?;
                            created_posts.push(created);
                        }
                        Ok(None) => {
                            println!("Warning: No available slot found for SNS: {}", sns_name);
                        }
                        Err(e) => {
                            println!("Error calculating slot for SNS {}: {:?}", sns_name, e);
                        }
                    }
                }

                if created_posts.is_empty() {
                    println!("Error: Failed to schedule post on any SNS via auto-slot.");
                    return Ok(());
                }

                println!(
                    "Successfully scheduled {} posts via auto-slot:",
                    created_posts.len()
                );
                for p in created_posts {
                    println!(
                        "  - ID: {} | Time: {} | SNS: {:?}",
                        p.id,
                        p.scheduled_at.format("%Y-%m-%d %H:%M:%S"),
                        p.target_sns
                    );
                }
                return Ok(());
            } else if let Some(at_str) = at {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&at_str) {
                    dt.with_timezone(&chrono::Local)
                } else if let Ok(dt) =
                    chrono::NaiveDateTime::parse_from_str(&at_str, "%Y-%m-%d %H:%M:%S")
                {
                    chrono::Local.from_local_datetime(&dt).unwrap()
                } else if let Ok(dt) =
                    chrono::NaiveDateTime::parse_from_str(&at_str, "%Y-%m-%d %H:%M")
                {
                    chrono::Local.from_local_datetime(&dt).unwrap()
                } else {
                    println!(
                        "Error: Invalid datetime format. Use RFC3339 (e.g., 2026-06-20T15:00:00+09:00) or 'YYYY-MM-DD HH:MM:SS'"
                    );
                    return Ok(());
                }
            } else {
                println!("Error: Either --at or --auto-slot must be specified.");
                return Ok(());
            };

            let mut post =
                scheduled::ScheduledPost::new(text, scheduled_time, processed_media, target_sns);
            post.link_url = link;

            let created = scheduled_store.create_post(post).await?;
            println!("Successfully scheduled post:");
            println!("  ID: {}", created.id);
            println!(
                "  Time: {}",
                created.scheduled_at.format("%Y-%m-%d %H:%M:%S")
            );
            println!("  SNS: {:?}", created.target_sns);
        }
        ScheduleAction::Delete { id } => {
            let success = scheduled_store.delete_post(&id).await?;
            if success {
                println!("Successfully deleted scheduled post: {}", id);
            } else {
                println!("Error: Scheduled post not found: {}", id);
            }
        }
        ScheduleAction::Update {
            id,
            text,
            at,
            sns,
            status,
            link,
        } => {
            let opt_post = scheduled_store.get_post_by_id(&id).await?;
            let Some(mut post) = opt_post else {
                println!("Error: Scheduled post not found: {}", id);
                return Ok(());
            };

            if let Some(t) = text {
                post.content = t;
            }
            use chrono::TimeZone;
            if let Some(at_str) = at {
                let parsed_time = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&at_str) {
                    dt.with_timezone(&chrono::Local)
                } else if let Ok(dt) =
                    chrono::NaiveDateTime::parse_from_str(&at_str, "%Y-%m-%d %H:%M:%S")
                {
                    chrono::Local.from_local_datetime(&dt).unwrap()
                } else if let Ok(dt) =
                    chrono::NaiveDateTime::parse_from_str(&at_str, "%Y-%m-%d %H:%M")
                {
                    chrono::Local.from_local_datetime(&dt).unwrap()
                } else {
                    println!(
                        "Error: Invalid datetime format. Use RFC3339 or 'YYYY-MM-DD HH:MM:SS'"
                    );
                    return Ok(());
                };
                post.scheduled_at = parsed_time;
            }
            if let Some(sns_arg) = sns {
                let mut target_sns = Vec::new();
                for part in sns_arg.split(',') {
                    let part = part.trim();
                    if !part.is_empty() {
                        target_sns.push(part.to_string());
                    }
                }
                post.target_sns = target_sns;
            }
            if let Some(s) = status {
                post.status = s;
            }
            if let Some(l) = link {
                post.link_url = Some(l);
            }

            post.updated_at = chrono::Local::now();

            let updated = scheduled_store.update_post(&id, post).await?;
            if let Some(p) = updated {
                println!("Successfully updated scheduled post: {}", p.id);
                println!("  Time: {}", p.scheduled_at.format("%Y-%m-%d %H:%M:%S"));
                println!("  SNS: {:?}", p.target_sns);
                println!("  Status: {}", p.status);
            } else {
                println!("Error: Failed to update scheduled post: {}", id);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use blog_autopost_rs::config::SnsConfig;
    use blog_autopost_rs::scheduled::JsonScheduledPostStore;
    use std::collections::HashMap;
    use tempfile::TempDir;

    /// テスト用の一時作業領域。
    ///
    /// `TempDir` はドロップ時に実体ごと削除されるため、テスト終了まで
    /// 保持し続ける必要がある。
    struct TestEnv {
        _dir: TempDir,
        store_path: String,
        uploads_dir: String,
    }

    impl TestEnv {
        fn new() -> Self {
            let dir = TempDir::new().expect("一時ディレクトリの作成に失敗");
            let store_path = dir
                .path()
                .join("scheduled_posts.json")
                .to_string_lossy()
                .into_owned();
            let uploads_dir = dir.path().join("uploads").to_string_lossy().into_owned();
            Self {
                _dir: dir,
                store_path,
                uploads_dir,
            }
        }

        fn store(&self) -> JsonScheduledPostStore {
            JsonScheduledPostStore::new(self.store_path.clone())
        }

        /// 一時作業領域のパス。テスト用の入力ファイルを置く場所として使う。
        ///
        /// `_dir` は生存期間の保持だけが役割なので直接は触らず、
        /// `store_path` の親から辿る。
        fn work_dir(&self) -> &std::path::Path {
            std::path::Path::new(&self.store_path)
                .parent()
                .expect("store_path には必ず親ディレクトリがある")
        }

        async fn run(&self, action: ScheduleAction, config: &Config) -> anyhow::Result<()> {
            run_with_paths(action, config, &self.store_path, &self.uploads_dir).await
        }

        async fn all_posts(&self) -> Vec<scheduled::ScheduledPost> {
            self.store()
                .get_all_posts()
                .await
                .expect("予約の取得に失敗")
        }
    }

    /// SNSを2件持つ設定を作る。
    fn config_with_sns() -> Config {
        Config {
            announcement_text: None,
            blog: None,
            sns: vec![
                SnsConfig::Mastodon {
                    name: "mstdn-main".to_string(),
                    instance_url: "https://mstdn.example.com".to_string(),
                    access_token: "t".to_string(),
                },
                SnsConfig::Misskey {
                    name: "misskey-main".to_string(),
                    instance_url: "https://misskey.example.com".to_string(),
                    access_token: "t".to_string(),
                    is_sensitive: None,
                },
            ],
            templates: HashMap::new(),
            default_allowed_timings: None,
            allowed_timings_tolerance_minutes: None,
            allowed_timings: None,
            web_auth: None,
            extra: HashMap::new(),
        }
    }

    /// SNS設定を持たない空の設定を作る。
    fn empty_config() -> Config {
        Config {
            sns: vec![],
            ..config_with_sns()
        }
    }

    fn add_action(text: &str, at: Option<&str>, sns: Option<&str>) -> ScheduleAction {
        ScheduleAction::Add {
            text: text.to_string(),
            at: at.map(|s| s.to_string()),
            auto_slot: false,
            sns: sns.map(|s| s.to_string()),
            media: None,
            link: None,
        }
    }

    // --- Add ---

    /// RFC3339 形式の日時で予約を追加できる。
    #[tokio::test]
    async fn test_add_with_rfc3339() {
        let env = TestEnv::new();

        env.run(
            add_action(
                "テスト投稿",
                Some("2026-08-01T12:00:00+09:00"),
                Some("bluesky"),
            ),
            &config_with_sns(),
        )
        .await
        .expect("追加に失敗");

        let posts = env.all_posts().await;
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].content, "テスト投稿");
        assert_eq!(posts[0].target_sns, vec!["bluesky"]);
        assert_eq!(posts[0].status, "予約済み");
    }

    /// "YYYY-MM-DD HH:MM" 形式でも予約を追加できる。
    #[tokio::test]
    async fn test_add_with_short_datetime() {
        let env = TestEnv::new();

        env.run(
            add_action("短い形式", Some("2026-08-01 12:00"), Some("bluesky")),
            &config_with_sns(),
        )
        .await
        .unwrap();

        let posts = env.all_posts().await;
        assert_eq!(posts.len(), 1);
        assert_eq!(
            posts[0].scheduled_at.format("%Y-%m-%d %H:%M").to_string(),
            "2026-08-01 12:00"
        );
    }

    /// "YYYY-MM-DD HH:MM:SS" 形式でも予約を追加できる。
    #[tokio::test]
    async fn test_add_with_seconds_datetime() {
        let env = TestEnv::new();

        env.run(
            add_action("秒あり", Some("2026-08-01 12:00:30"), Some("bluesky")),
            &config_with_sns(),
        )
        .await
        .unwrap();

        assert_eq!(env.all_posts().await.len(), 1);
    }

    /// SNS を省略すると設定内の全SNSが対象になる。
    #[tokio::test]
    async fn test_add_without_sns_uses_all_configured() {
        let env = TestEnv::new();

        env.run(
            add_action("全SNS向け", Some("2026-08-01T12:00:00+09:00"), None),
            &config_with_sns(),
        )
        .await
        .unwrap();

        let posts = env.all_posts().await;
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].target_sns, vec!["mstdn-main", "misskey-main"]);
    }

    /// カンマ区切りで複数SNSを指定できる。空要素は無視される。
    #[tokio::test]
    async fn test_add_with_multiple_sns() {
        let env = TestEnv::new();

        env.run(
            add_action(
                "複数SNS",
                Some("2026-08-01T12:00:00+09:00"),
                Some("bluesky, ,mastodon"),
            ),
            &config_with_sns(),
        )
        .await
        .unwrap();

        assert_eq!(
            env.all_posts().await[0].target_sns,
            vec!["bluesky", "mastodon"]
        );
    }

    /// SNSが1つも決まらない場合は追加されない。
    #[tokio::test]
    async fn test_add_without_any_sns_creates_nothing() {
        let env = TestEnv::new();

        env.run(
            add_action("対象なし", Some("2026-08-01T12:00:00+09:00"), None),
            &empty_config(),
        )
        .await
        .expect("エラーではなくメッセージ表示で終わる");

        assert!(env.all_posts().await.is_empty());
    }

    /// 不正な日時形式では追加されない。
    #[tokio::test]
    async fn test_add_with_invalid_datetime_creates_nothing() {
        let env = TestEnv::new();

        env.run(
            add_action("不正な日時", Some("いつか"), Some("bluesky")),
            &config_with_sns(),
        )
        .await
        .expect("エラーではなくメッセージ表示で終わる");

        assert!(env.all_posts().await.is_empty());
    }

    /// --at も --auto-slot も無い場合は追加されない。
    #[tokio::test]
    async fn test_add_without_time_creates_nothing() {
        let env = TestEnv::new();

        env.run(
            add_action("時刻なし", None, Some("bluesky")),
            &config_with_sns(),
        )
        .await
        .unwrap();

        assert!(env.all_posts().await.is_empty());
    }

    /// 存在しないメディアファイルを指定した場合は追加されない。
    #[tokio::test]
    async fn test_add_with_missing_media_creates_nothing() {
        let env = TestEnv::new();

        let action = ScheduleAction::Add {
            text: "メディア付き".to_string(),
            at: Some("2026-08-01T12:00:00+09:00".to_string()),
            auto_slot: false,
            sns: Some("bluesky".to_string()),
            media: Some(vec!["/存在しないパス/x.png".to_string()]),
            link: None,
        };

        env.run(action, &config_with_sns()).await.unwrap();

        assert!(env.all_posts().await.is_empty());
    }

    /// メディアはアップロード先へ退避され、退避後のパスが記録される。
    #[tokio::test]
    async fn test_add_copies_media_to_uploads_dir() {
        let env = TestEnv::new();
        let src = env.work_dir().join("元画像.png");
        std::fs::write(&src, b"dummy image").expect("テスト画像の作成に失敗");

        let action = ScheduleAction::Add {
            text: "メディア付き".to_string(),
            at: Some("2026-08-01T12:00:00+09:00".to_string()),
            auto_slot: false,
            sns: Some("bluesky".to_string()),
            media: Some(vec![src.to_string_lossy().into_owned()]),
            link: Some("https://example.com/a".to_string()),
        };

        env.run(action, &config_with_sns()).await.unwrap();

        let posts = env.all_posts().await;
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].media_files.len(), 1);

        let saved = &posts[0].media_files[0];
        assert!(
            saved.starts_with(&env.uploads_dir),
            "退避先が違う: {}",
            saved
        );
        assert!(
            std::path::Path::new(saved).exists(),
            "退避したファイルが無い"
        );
        // 日本語のファイル名はアンダースコアへ置き換えられる
        assert!(saved.ends_with(".png"), "拡張子が保たれていない: {}", saved);
        assert_eq!(posts[0].link_url.as_deref(), Some("https://example.com/a"));
    }

    // --- List ---

    /// 一覧はエラーなく実行できる(空でも可)。
    #[tokio::test]
    async fn test_list_empty() {
        let env = TestEnv::new();

        env.run(ScheduleAction::List { status: None }, &config_with_sns())
            .await
            .expect("一覧の取得に失敗");
    }

    /// ステータス指定つきの一覧もエラーなく実行できる。
    #[tokio::test]
    async fn test_list_with_status_filter() {
        let env = TestEnv::new();
        seed(&env, "予約1", "予約済み").await;
        seed(&env, "予約2", "投稿済み").await;

        env.run(
            ScheduleAction::List {
                status: Some("予約済み".to_string()),
            },
            &config_with_sns(),
        )
        .await
        .unwrap();

        // フィルタは表示のみに作用し、保存内容は変わらない
        assert_eq!(env.all_posts().await.len(), 2);
    }

    /// 30文字を超える本文はプレビューが省略される経路を通る。
    #[tokio::test]
    async fn test_list_with_long_content() {
        let env = TestEnv::new();
        seed(&env, &"あ".repeat(50), "予約済み").await;

        env.run(ScheduleAction::List { status: None }, &config_with_sns())
            .await
            .unwrap();
    }

    // --- Delete ---

    #[tokio::test]
    async fn test_delete_existing_post() {
        let env = TestEnv::new();
        let id = seed(&env, "消す予約", "予約済み").await;

        env.run(
            ScheduleAction::Delete { id: id.clone() },
            &config_with_sns(),
        )
        .await
        .unwrap();

        assert!(env.all_posts().await.is_empty());
    }

    #[tokio::test]
    async fn test_delete_missing_post_is_not_an_error() {
        let env = TestEnv::new();
        seed(&env, "残る予約", "予約済み").await;

        env.run(
            ScheduleAction::Delete {
                id: "post-does-not-exist".to_string(),
            },
            &config_with_sns(),
        )
        .await
        .expect("存在しないIDでもErrにはならない");

        assert_eq!(env.all_posts().await.len(), 1);
    }

    // --- Update ---

    #[tokio::test]
    async fn test_update_all_fields() {
        let env = TestEnv::new();
        let id = seed(&env, "変更前", "予約済み").await;

        env.run(
            ScheduleAction::Update {
                id: id.clone(),
                text: Some("変更後".to_string()),
                at: Some("2026-09-01T09:00:00+09:00".to_string()),
                sns: Some("mastodon,bluesky".to_string()),
                status: Some("投稿済み".to_string()),
                link: Some("https://example.com/b".to_string()),
            },
            &config_with_sns(),
        )
        .await
        .unwrap();

        let posts = env.all_posts().await;
        assert_eq!(posts.len(), 1);
        let p = &posts[0];
        assert_eq!(p.content, "変更後");
        assert_eq!(p.target_sns, vec!["mastodon", "bluesky"]);
        assert_eq!(p.status, "投稿済み");
        assert_eq!(p.link_url.as_deref(), Some("https://example.com/b"));
        assert_eq!(
            p.scheduled_at.format("%Y-%m-%d %H:%M").to_string(),
            "2026-09-01 09:00"
        );
    }

    /// 指定しなかった項目は変更されない。
    #[tokio::test]
    async fn test_update_only_text_keeps_other_fields() {
        let env = TestEnv::new();
        let id = seed(&env, "元の本文", "予約済み").await;
        let before = env.all_posts().await[0].clone();

        env.run(
            ScheduleAction::Update {
                id: id.clone(),
                text: Some("新しい本文".to_string()),
                at: None,
                sns: None,
                status: None,
                link: None,
            },
            &config_with_sns(),
        )
        .await
        .unwrap();

        let after = env.all_posts().await[0].clone();
        assert_eq!(after.content, "新しい本文");
        assert_eq!(after.target_sns, before.target_sns);
        assert_eq!(after.status, before.status);
        assert_eq!(after.scheduled_at, before.scheduled_at);
    }

    /// 存在しないIDの更新はエラーにならず、何も変わらない。
    #[tokio::test]
    async fn test_update_missing_post_is_not_an_error() {
        let env = TestEnv::new();
        seed(&env, "そのまま", "予約済み").await;

        env.run(
            ScheduleAction::Update {
                id: "post-does-not-exist".to_string(),
                text: Some("届かない変更".to_string()),
                at: None,
                sns: None,
                status: None,
                link: None,
            },
            &config_with_sns(),
        )
        .await
        .expect("存在しないIDでもErrにはならない");

        assert_eq!(env.all_posts().await[0].content, "そのまま");
    }

    /// 不正な日時での更新は行われない。
    #[tokio::test]
    async fn test_update_with_invalid_datetime_changes_nothing() {
        let env = TestEnv::new();
        let id = seed(&env, "元の本文", "予約済み").await;

        env.run(
            ScheduleAction::Update {
                id: id.clone(),
                text: Some("適用されない".to_string()),
                at: Some("めちゃくちゃな日時".to_string()),
                sns: None,
                status: None,
                link: None,
            },
            &config_with_sns(),
        )
        .await
        .unwrap();

        // 日時のパースに失敗した時点で処理を打ち切るため、本文も変わらない
        assert_eq!(env.all_posts().await[0].content, "元の本文");
    }

    /// 予約を1件用意し、そのIDを返す。
    async fn seed(env: &TestEnv, content: &str, status: &str) -> String {
        let mut post = scheduled::ScheduledPost::new(
            content.to_string(),
            chrono::Local::now() + chrono::Duration::hours(1),
            vec![],
            vec!["bluesky".to_string()],
        );
        post.status = status.to_string();
        let created = env
            .store()
            .create_post(post)
            .await
            .expect("予約の作成に失敗");
        created.id
    }
}
