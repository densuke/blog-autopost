//! MCP が公開する tool の定義と実行。
//!
//! tool の一覧は [`tool_definitions`] が返し、実行は [`handle_tool_call`] が
//! 名前で分岐して行う。JSON-RPC の組み立ては [`super::protocol`] の責務であり、
//! ここでは tool 単体の入出力だけを扱う。

use serde_json::json;
use std::sync::Arc;

use crate::web::AppState;

/// MCP が公開する tool の定義一覧を返す。
///
/// `tools/list` のレスポンスにそのまま載る JSON 配列であり、
/// 各要素は `name` / `description` / `inputSchema` を持つ。
pub(crate) fn tool_definitions() -> Vec<serde_json::Value> {
    vec![
        json!({
            "name": "list_schedules",
            "description": "予約投稿の一覧を取得します。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "description": "特定のステータスでフィルタ（'予約済み', '投稿済み', '失敗'）"
                    }
                }
            }
        }),
        json!({
            "name": "add_schedule",
            "description": "新しく予約投稿を追加します。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "投稿するメッセージ本文"
                    },
                    "at": {
                        "type": "string",
                        "description": "投稿予定時刻 (RFC3339形式。例: '2026-06-20T18:00:00+09:00')"
                    },
                    "auto_slot": {
                        "type": "boolean",
                        "description": "空いている最適な次の投稿可能時間枠を自動検索する"
                    },
                    "sns": {
                        "type": "string",
                        "description": "投稿先SNS名（カンマ区切り。省略時は全SNS）"
                    },
                    "media": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "添付するローカルの画像ファイルパス"
                    },
                    "link": {
                        "type": "string",
                        "description": "添付するリンクURL"
                    }
                },
                "required": ["text"]
            }
        }),
        json!({
            "name": "update_schedule",
            "description": "既存の予約投稿の内容や日時を変更します。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "変更対象の予約投稿ID" },
                    "text": { "type": "string", "description": "変更後の本文" },
                    "at": { "type": "string", "description": "変更後の予定時刻 (RFC3339形式)" },
                    "sns": { "type": "string", "description": "変更後のSNS名" },
                    "status": { "type": "string", "description": "変更後のステータス" },
                    "link": { "type": "string", "description": "変更後のリンクURL" }
                },
                "required": ["id"]
            }
        }),
        json!({
            "name": "delete_schedule",
            "description": "指定したIDの予約投稿を削除します。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "削除対象の予約投稿ID" }
                },
                "required": ["id"]
            }
        }),
        json!({
            "name": "post_now",
            "description": "今すぐ指定のSNSへ直接手動投稿します（予約せず直ちに投稿）。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "投稿メッセージ本文" },
                    "sns": { "type": "string", "description": "送信先SNS名（カンマ区切り。省略時は全SNS）" },
                    "media": { "type": "array", "items": { "type": "string" }, "description": "添付するローカル画像パス" },
                    "link": { "type": "string", "description": "添付するリンクURL" }
                },
                "required": ["text"]
            }
        }),
    ]
}

/// 名前で tool を選び、実行結果を人間が読める文字列として返す。
///
/// 未知の tool 名や引数不足はエラーとして返し、呼び出し側が
/// JSON-RPC のエラーレスポンスへ変換する。
pub(crate) async fn handle_tool_call(
    state: Arc<AppState>,
    name: &str,
    args: serde_json::Value,
) -> anyhow::Result<String> {
    match name {
        "list_schedules" => {
            let status_filter = args.get("status").and_then(|s| s.as_str());
            let posts = state.store.get_all_posts().await?;
            let mut filtered = posts;
            if let Some(s) = status_filter {
                filtered.retain(|p| p.status == s);
            }
            filtered.sort_by_key(|p| p.scheduled_at);

            let mut out = String::new();
            out.push_str("=== 予約投稿一覧 ===\n");
            for p in filtered {
                out.push_str(&format!(
                    "ID: {} | Time: {} | SNS: {:?} | Status: {} | Text: {}\n",
                    p.id,
                    p.scheduled_at.format("%Y-%m-%d %H:%M:%S"),
                    p.target_sns,
                    p.status,
                    if p.content.chars().count() > 40 {
                        format!("{}...", p.content.chars().take(37).collect::<String>())
                    } else {
                        p.content.clone()
                    }
                ));
            }
            if out.lines().count() == 1 {
                out.push_str("(予約投稿はありません)\n");
            }
            Ok(out)
        }
        "add_schedule" => {
            let text = args
                .get("text")
                .and_then(|t| t.as_str())
                .ok_or_else(|| anyhow::anyhow!("text is required"))?
                .to_string();
            let at = args.get("at").and_then(|a| a.as_str());
            let auto_slot = args
                .get("auto_slot")
                .and_then(|a| a.as_bool())
                .unwrap_or(false);
            let sns = args.get("sns").and_then(|s| s.as_str());
            let media = args.get("media").and_then(|m| m.as_array());
            let link = args
                .get("link")
                .and_then(|l| l.as_str())
                .map(|s| s.to_string());

            let mut target_sns = Vec::new();
            if let Some(sns_arg) = sns {
                for part in sns_arg.split(',') {
                    let part = part.trim();
                    if !part.is_empty() {
                        target_sns.push(part.to_string());
                    }
                }
            } else {
                for sns_conf in &state.config.sns {
                    let name = match sns_conf {
                        crate::config::SnsConfig::Mastodon { name, .. } => name,
                        crate::config::SnsConfig::Misskey { name, .. } => name,
                        crate::config::SnsConfig::Bluesky { name, .. } => name,
                        crate::config::SnsConfig::X { name, .. } => name,
                        crate::config::SnsConfig::Threads { name, .. } => name,
                        crate::config::SnsConfig::Tumblr { name, .. } => name,
                        _ => continue,
                    };
                    target_sns.push(name.clone());
                }
            }

            if target_sns.is_empty() {
                return Err(anyhow::anyhow!("No target SNS configured or specified"));
            }

            let mut processed_media = Vec::new();
            if let Some(media_list) = media {
                std::fs::create_dir_all("data/uploads").ok();
                for val in media_list {
                    if let Some(file_path) = val.as_str() {
                        let path = std::path::Path::new(file_path);
                        if !path.exists() {
                            return Err(anyhow::anyhow!("Media file not found: {}", file_path));
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
                        std::fs::copy(file_path, &save_path)?;
                        processed_media.push(save_path);
                    }
                }
            }

            use chrono::TimeZone;
            if auto_slot {
                let finder = crate::timing::SlotFinder::new(&state.timing_manager, &state.store, 5);
                let mut created_posts = Vec::new();
                for sns_name in &target_sns {
                    if let Some(dt) = finder.find_next_available_slot(sns_name, None, 7).await? {
                        let mut post = crate::scheduled::ScheduledPost::new(
                            text.clone(),
                            dt,
                            processed_media.clone(),
                            vec![sns_name.clone()],
                        );
                        post.link_url = link.clone();
                        let created = state.store.create_post(post).await?;
                        created_posts.push(created);
                    }
                }
                let mut out = format!(
                    "Successfully scheduled {} posts via auto-slot:\n",
                    created_posts.len()
                );
                for p in created_posts {
                    out.push_str(&format!(
                        "  - ID: {} | Time: {} | SNS: {:?}\n",
                        p.id,
                        p.scheduled_at.format("%Y-%m-%d %H:%M:%S"),
                        p.target_sns
                    ));
                }
                Ok(out)
            } else if let Some(at_str) = at {
                let parsed_time = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(at_str) {
                    dt.with_timezone(&chrono::Local)
                } else if let Ok(dt) =
                    chrono::NaiveDateTime::parse_from_str(at_str, "%Y-%m-%d %H:%M:%S")
                {
                    chrono::Local.from_local_datetime(&dt).unwrap()
                } else if let Ok(dt) =
                    chrono::NaiveDateTime::parse_from_str(at_str, "%Y-%m-%d %H:%M")
                {
                    chrono::Local.from_local_datetime(&dt).unwrap()
                } else {
                    return Err(anyhow::anyhow!(
                        "Invalid datetime format. Use RFC3339 or 'YYYY-MM-DD HH:MM:SS'"
                    ));
                };

                let mut post = crate::scheduled::ScheduledPost::new(
                    text,
                    parsed_time,
                    processed_media,
                    target_sns,
                );
                post.link_url = link;
                let created = state.store.create_post(post).await?;
                Ok(format!(
                    "Successfully scheduled post:\n  ID: {}\n  Time: {}\n  SNS: {:?}",
                    created.id,
                    created.scheduled_at.format("%Y-%m-%d %H:%M:%S"),
                    created.target_sns
                ))
            } else {
                Err(anyhow::anyhow!(
                    "Either 'at' or 'auto_slot' must be specified"
                ))
            }
        }
        "update_schedule" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| anyhow::anyhow!("id is required"))?;
            let opt_post = state.store.get_post_by_id(id).await?;
            let mut post = opt_post.ok_or_else(|| anyhow::anyhow!("Scheduled post not found"))?;

            if let Some(t) = args.get("text").and_then(|t| t.as_str()) {
                post.content = t.to_string();
            }
            use chrono::TimeZone;
            if let Some(at_str) = args.get("at").and_then(|a| a.as_str()) {
                let parsed_time = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(at_str) {
                    dt.with_timezone(&chrono::Local)
                } else if let Ok(dt) =
                    chrono::NaiveDateTime::parse_from_str(at_str, "%Y-%m-%d %H:%M:%S")
                {
                    chrono::Local.from_local_datetime(&dt).unwrap()
                } else if let Ok(dt) =
                    chrono::NaiveDateTime::parse_from_str(at_str, "%Y-%m-%d %H:%M")
                {
                    chrono::Local.from_local_datetime(&dt).unwrap()
                } else {
                    return Err(anyhow::anyhow!("Invalid datetime format"));
                };
                post.scheduled_at = parsed_time;
            }
            if let Some(sns_arg) = args.get("sns").and_then(|s| s.as_str()) {
                let mut target_sns = Vec::new();
                for part in sns_arg.split(',') {
                    let part = part.trim();
                    if !part.is_empty() {
                        target_sns.push(part.to_string());
                    }
                }
                post.target_sns = target_sns;
            }
            if let Some(s) = args.get("status").and_then(|s| s.as_str()) {
                post.status = s.to_string();
            }
            if let Some(l) = args.get("link").and_then(|l| l.as_str()) {
                post.link_url = Some(l.to_string());
            }

            post.updated_at = chrono::Local::now();
            let updated = state.store.update_post(id, post).await?;
            if let Some(p) = updated {
                Ok(format!(
                    "Successfully updated scheduled post: {}\n  Time: {}\n  SNS: {:?}\n  Status: {}",
                    p.id,
                    p.scheduled_at.format("%Y-%m-%d %H:%M:%S"),
                    p.target_sns,
                    p.status
                ))
            } else {
                Err(anyhow::anyhow!("Failed to update scheduled post"))
            }
        }
        "delete_schedule" => {
            let id = args
                .get("id")
                .and_then(|i| i.as_str())
                .ok_or_else(|| anyhow::anyhow!("id is required"))?;
            let success = state.store.delete_post(id).await?;
            if success {
                Ok(format!("Successfully deleted scheduled post: {}", id))
            } else {
                Err(anyhow::anyhow!("Scheduled post not found"))
            }
        }
        "post_now" => {
            let text = args
                .get("text")
                .and_then(|t| t.as_str())
                .ok_or_else(|| anyhow::anyhow!("text is required"))?
                .to_string();
            let sns = args.get("sns").and_then(|s| s.as_str());
            let media = args.get("media").and_then(|m| m.as_array());
            let link = args
                .get("link")
                .and_then(|l| l.as_str())
                .map(|s| s.to_string());

            let mut sns_clients: Vec<Box<dyn crate::sns::traits::SnsClient + Send + Sync>> =
                Vec::new();

            let mut included = std::collections::HashSet::new();
            if let Some(sns_arg) = sns {
                for part in sns_arg.split(',') {
                    let part = part.trim().to_lowercase();
                    if !part.is_empty() {
                        included.insert(part);
                    }
                }
            }

            for sns_conf in &state.config.sns {
                let name = match sns_conf {
                    crate::config::SnsConfig::Mastodon { name, .. } => name,
                    crate::config::SnsConfig::Misskey { name, .. } => name,
                    crate::config::SnsConfig::Bluesky { name, .. } => name,
                    crate::config::SnsConfig::X { name, .. } => name,
                    _ => continue,
                };

                if !included.is_empty() {
                    let lower_name = name.to_lowercase();
                    let lower_type = match sns_conf {
                        crate::config::SnsConfig::Mastodon { .. } => "mastodon",
                        crate::config::SnsConfig::Misskey { .. } => "misskey",
                        crate::config::SnsConfig::Bluesky { .. } => "bluesky",
                        crate::config::SnsConfig::X { .. } => "x",
                        _ => "",
                    };
                    if !included.contains(&lower_name) && !included.contains(lower_type) {
                        continue;
                    }
                }

                match sns_conf {
                    crate::config::SnsConfig::Mastodon {
                        instance_url,
                        access_token,
                        name,
                        ..
                    } => {
                        if let Ok(c) = crate::sns::mastodon::MastodonClient::new(
                            instance_url.clone(),
                            access_token.clone(),
                            name.clone(),
                        ) {
                            sns_clients.push(Box::new(c));
                        }
                    }
                    crate::config::SnsConfig::Misskey {
                        instance_url,
                        access_token,
                        name,
                        ..
                    } => {
                        if let Ok(c) = crate::sns::misskey::MisskeyClient::new(
                            instance_url.clone(),
                            access_token.clone(),
                            name.clone(),
                        ) {
                            sns_clients.push(Box::new(c));
                        }
                    }
                    crate::config::SnsConfig::Bluesky {
                        identifier,
                        password,
                        name,
                        ..
                    } => {
                        if let Ok(c) = crate::sns::bluesky::BlueskyClient::new(
                            identifier.clone(),
                            password.clone(),
                            name.clone(),
                        ) {
                            sns_clients.push(Box::new(c));
                        }
                    }
                    crate::config::SnsConfig::X {
                        consumer_key,
                        consumer_secret,
                        access_token,
                        access_token_secret,
                        name,
                    } => {
                        if let Ok(c) = crate::sns::x::XClient::new(
                            consumer_key.clone(),
                            consumer_secret.clone(),
                            access_token.clone(),
                            access_token_secret.clone(),
                            name.clone(),
                        ) {
                            sns_clients.push(Box::new(c));
                        }
                    }
                    _ => {}
                }
            }

            if sns_clients.is_empty() {
                return Err(anyhow::anyhow!(
                    "No active SNS client matched target: {:?}",
                    sns
                ));
            }

            let mut processed_media = Vec::new();
            if let Some(media_list) = media {
                for val in media_list {
                    if let Some(s) = val.as_str() {
                        processed_media.push(s.to_string());
                    }
                }
            }

            let post_content = crate::sns::models::PostContent {
                text,
                image_url: None,
                media_paths: if processed_media.is_empty() {
                    None
                } else {
                    Some(processed_media)
                },
                link_url: link,
                sensitive: false,
            };

            let mut out = String::new();
            out.push_str("=== 投稿実行結果 ===\n");
            for client in sns_clients {
                out.push_str(&format!("Posting to {}...\n", client.name()));
                match client.post(&post_content).await {
                    Ok(res) => {
                        if res.success {
                            out.push_str(&format!("  [Success] ID: {:?}\n", res.post_id));
                        } else {
                            out.push_str(&format!("  [Failed] Error: {:?}\n", res.error_message));
                        }
                    }
                    Err(e) => {
                        out.push_str(&format!("  [Error] {:?}\n", e));
                    }
                }
            }
            Ok(out)
        }
        _ => Err(anyhow::anyhow!("Unknown tool name: {}", name)),
    }
}
