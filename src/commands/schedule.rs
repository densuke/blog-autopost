use blog_autopost_rs::config::{self, Config};
use blog_autopost_rs::{scheduled, timing};

use crate::cli::ScheduleAction;

/// `schedule` サブコマンド（一覧・追加・削除・変更）を処理する。
pub async fn run(action: ScheduleAction, config_data: &Config) -> anyhow::Result<()> {
    let scheduled_store = scheduled::JsonScheduledPostStore::new("data/scheduled_posts.json");
    std::fs::create_dir_all("data").ok();

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
                if let Err(e) = std::fs::create_dir_all("data/uploads") {
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
                    let save_path = format!("data/uploads/{}", unique_name);

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
