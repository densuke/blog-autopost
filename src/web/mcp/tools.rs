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
                        "description": "添付するローカルの画像ファイルパス（許可ディレクトリ配下のみ）"
                    },
                    "link": {
                        "type": "string",
                        "description": "添付するリンクURL"
                    },
                    "sensitive": {
                        "type": "boolean",
                        "description": "添付メディアをセンシティブとして扱う（現状 Misskey のみ有効）"
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
                    "media": { "type": "array", "items": { "type": "string" }, "description": "添付するローカル画像パス（許可ディレクトリ配下のみ）" },
                    "link": { "type": "string", "description": "添付するリンクURL" },
                    "sensitive": { "type": "boolean", "description": "添付メディアをセンシティブとして扱う（現状 Misskey のみ有効）" }
                },
                "required": ["text"]
            }
        }),
        json!({
            "name": "get_next_slots",
            "description": "各SNSの次に投稿可能な時間枠を取得します。予約せず照会するだけです。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "sns": {
                        "type": "string",
                        "description": "対象SNS名（カンマ区切り。省略時は全SNS）"
                    }
                }
            }
        }),
    ]
}

/// `sns` 引数のカンマ区切りを分解する。
///
/// 未指定なら `None` を返す。空要素と前後の空白は取り除く。
/// 分解した結果が空になった場合も未指定として扱う。
fn parse_sns_targets(sns: Option<&str>) -> Option<Vec<String>> {
    let targets: Vec<String> = sns?
        .split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect();

    if targets.is_empty() {
        None
    } else {
        Some(targets)
    }
}

/// tool の `media` 引数を検証し、解決済みの絶対パスを返す。
///
/// 許可ディレクトリの外や、対応していない形式のファイルは弾く。
/// 検証を通さないと、認証済みのクライアントが任意のファイルを
/// SNS へ送信できてしまう。
fn validate_media_args(
    state: &Arc<AppState>,
    media: Option<&Vec<serde_json::Value>>,
) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let Some(media_list) = media else {
        return Ok(Vec::new());
    };

    let allowed = allowed_media_dirs(state);
    let mut resolved = Vec::new();
    for val in media_list {
        if let Some(file_path) = val.as_str() {
            resolved.push(crate::web::media::validate_media_path(
                &allowed,
                file_path,
                crate::web::media::MAX_MEDIA_BYTES,
            )?);
        }
    }
    Ok(resolved)
}

/// 検証済みのメディアをアップロード領域へ複製し、その保存先を返す。
fn copy_media_to_uploads(
    state: &Arc<AppState>,
    media: Option<&Vec<serde_json::Value>>,
) -> anyhow::Result<Vec<String>> {
    let sources = validate_media_args(state, media)?;
    if sources.is_empty() {
        return Ok(Vec::new());
    }

    std::fs::create_dir_all(&state.upload_dir)?;

    let mut saved = Vec::new();
    for source in sources {
        let file_name = source
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("image.png");
        let save_path = state
            .upload_dir
            .join(crate::web::media::unique_file_name(file_name));
        std::fs::copy(&source, &save_path)?;
        saved.push(save_path.to_string_lossy().into_owned());
    }
    Ok(saved)
}

/// メディアとして参照してよいディレクトリの一覧を返す。
///
/// `mcp.allowed_media_dirs` があればそれを使い、無ければ既定値を使う。
/// アップロード領域は常に含める。Web UI から上げたファイルを
/// MCP から使えなくなると、通常の運用が成り立たないため。
fn allowed_media_dirs(state: &Arc<AppState>) -> Vec<std::path::PathBuf> {
    let mut dirs = super::auth::allowed_media_dirs(state.config.mcp.as_ref());
    if !dirs.contains(&state.upload_dir) {
        dirs.push(state.upload_dir.clone());
    }
    dirs
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
            let sensitive = args
                .get("sensitive")
                .and_then(|s| s.as_bool())
                .unwrap_or(false);

            let target_sns = match parse_sns_targets(sns) {
                Some(targets) => targets,
                None => {
                    // 未指定なら投稿できる種別だけを対象にする。
                    // Threads / Tumblr を含めても投稿時に落ちるだけなので入れない。
                    state
                        .config
                        .sns
                        .iter()
                        .filter(|s| crate::sns::is_postable_type(s))
                        .filter_map(|s| {
                            crate::web::routes::sns_account_name(s).map(|n| n.to_string())
                        })
                        .collect()
                }
            };

            if target_sns.is_empty() {
                return Err(anyhow::anyhow!("No target SNS configured or specified"));
            }

            // 予約時点でアップロード領域へ複製しておく。元ファイルが
            // 投稿時刻までに消えても投稿できるようにするため。
            let processed_media = copy_media_to_uploads(&state, media)?;

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
                        post.sensitive = sensitive;
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
                post.sensitive = sensitive;
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

            let sensitive = args
                .get("sensitive")
                .and_then(|s| s.as_bool())
                .unwrap_or(false);

            let targets = parse_sns_targets(sns);
            let (sns_clients, unsupported) =
                crate::sns::build_selected_clients(&state.config, targets.as_deref());

            // 指定したのに黙って無視されるのを避け、未対応であることを明示する
            if !unsupported.is_empty() {
                return Err(anyhow::anyhow!(
                    "These SNS accounts are configured but posting is not implemented \
                     in this build: {}. Remove them from the 'sns' argument.",
                    unsupported.join(", ")
                ));
            }

            if sns_clients.is_empty() {
                return Err(anyhow::anyhow!(
                    "No active SNS client matched target: {:?}",
                    sns
                ));
            }

            // 即時投稿でも検証は必須。従来は生のパスをそのまま
            // SNS クライアントへ渡しており、任意のファイルを送信できた。
            let processed_media = validate_media_args(&state, media)?
                .into_iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>();

            let post_content = crate::sns::models::PostContent {
                text,
                image_url: None,
                media_paths: if processed_media.is_empty() {
                    None
                } else {
                    Some(processed_media)
                },
                link_url: link,
                sensitive,
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
        "get_next_slots" => {
            let sns = args.get("sns").and_then(|s| s.as_str());
            let targets = parse_sns_targets(sns);

            let slots = crate::web::routes::collect_next_slots(&state, targets.as_deref()).await?;

            if slots.is_empty() {
                return Ok("=== 次の投稿枠 ===\n(対象のSNSがありません)\n".to_string());
            }

            let mut out = String::new();
            out.push_str("=== 次の投稿枠 ===\n");
            for (name, slot) in slots {
                match slot {
                    Some(dt) => out.push_str(&format!(
                        "{}: {}\n",
                        name,
                        dt.format("%Y-%m-%d %H:%M:%S %:z")
                    )),
                    // タイミング未設定や空き枠が見つからない場合
                    None => out.push_str(&format!("{}: (空き枠が見つかりません)\n", name)),
                }
            }
            Ok(out)
        }
        _ => Err(anyhow::anyhow!("Unknown tool name: {}", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SnsConfig;
    use crate::scheduled::ScheduledPost;
    use crate::web::tests::{TestApp, setup_test_app, setup_test_app_with_config};

    const SECRET: &str = "test-secret-token";

    fn app() -> TestApp {
        setup_test_app(Some(SECRET.to_string()))
    }

    /// 予約を1件用意し、そのIDを返す。
    async fn seed(app: &TestApp, content: &str, minutes_ahead: i64) -> String {
        let post = ScheduledPost::new(
            content.to_string(),
            chrono::Local::now() + chrono::Duration::minutes(minutes_ahead),
            vec![],
            vec!["bluesky".to_string()],
        );
        app.state
            .store
            .create_post(post)
            .await
            .expect("予約の作成に失敗")
            .id
    }

    /// tool を呼び出して結果テキストを取り出す。
    async fn call(app: &TestApp, name: &str, args: serde_json::Value) -> anyhow::Result<String> {
        handle_tool_call(app.state.clone(), name, args).await
    }

    // --- 定義 ---

    /// tool 定義は名前が一意で、必要な項目を備えている。
    #[test]
    fn tool定義は名前が一意で必要項目を備える() {
        let defs = tool_definitions();
        let names: Vec<&str> = defs.iter().filter_map(|d| d["name"].as_str()).collect();

        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "tool名が重複している");

        for d in &defs {
            assert!(d["name"].as_str().is_some_and(|s| !s.is_empty()));
            assert!(d["description"].as_str().is_some_and(|s| !s.is_empty()));
            assert_eq!(d["inputSchema"]["type"], "object");
        }
    }

    // --- list_schedules ---

    /// 予約が無いときは空である旨を返す。
    #[tokio::test]
    async fn list_schedules_は空のとき案内を返す() {
        let app = app();
        let out = call(&app, "list_schedules", json!({})).await.unwrap();

        assert!(out.contains("予約投稿一覧"));
        assert!(out.contains("(予約投稿はありません)"));
    }

    /// 予約は時刻の昇順で並ぶ。
    #[tokio::test]
    async fn list_schedules_は時刻の昇順で並ぶ() {
        let app = app();
        seed(&app, "あとの予約", 120).await;
        seed(&app, "さきの予約", 30).await;

        let out = call(&app, "list_schedules", json!({})).await.unwrap();

        let first = out.find("さきの予約").expect("先の予約が含まれる");
        let second = out.find("あとの予約").expect("後の予約が含まれる");
        assert!(first < second, "時刻の昇順で並ぶこと:\n{}", out);
    }

    /// status で絞り込める。
    #[tokio::test]
    async fn list_schedules_はstatusで絞り込める() {
        let app = app();
        let id = seed(&app, "投稿済みにする予約", 30).await;
        seed(&app, "予約済みのまま", 60).await;

        let mut post = app
            .state
            .store
            .get_post_by_id(&id)
            .await
            .unwrap()
            .expect("作成した予約が取れる");
        post.status = "投稿済み".to_string();
        app.state.store.update_post(&id, post).await.unwrap();

        let out = call(&app, "list_schedules", json!({ "status": "投稿済み" }))
            .await
            .unwrap();

        assert!(out.contains("投稿済みにする予約"));
        assert!(!out.contains("予約済みのまま"));
    }

    /// 40文字を超える本文は省略される。
    #[tokio::test]
    async fn list_schedules_は長い本文を省略する() {
        let app = app();
        let long = "あ".repeat(50);
        seed(&app, &long, 30).await;

        let out = call(&app, "list_schedules", json!({})).await.unwrap();

        assert!(out.contains("..."), "省略記号が付くこと:\n{}", out);
        assert!(!out.contains(&long), "全文は載らないこと");
    }

    // --- add_schedule ---

    /// text が無ければエラーになる。
    #[tokio::test]
    async fn add_schedule_はtext必須() {
        let app = app();
        let err = call(&app, "add_schedule", json!({ "at": "2030-01-01 09:00" }))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("text is required"));
    }

    /// at も auto_slot も無ければエラーになる。
    #[tokio::test]
    async fn add_schedule_はatもauto_slotも無ければエラー() {
        let app = app();
        let err = call(
            &app,
            "add_schedule",
            json!({ "text": "本文", "sns": "bluesky" }),
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string().contains("'at' or 'auto_slot'"),
            "実際のエラー: {}",
            err
        );
    }

    /// SNS の指定も設定も無ければエラーになる。
    #[tokio::test]
    async fn add_schedule_は対象snsが無ければエラー() {
        let app = app();
        let err = call(
            &app,
            "add_schedule",
            json!({ "text": "本文", "at": "2030-01-01 09:00" }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("No target SNS"));
    }

    /// RFC3339 形式の日時を受け付ける。
    #[tokio::test]
    async fn add_schedule_はrfc3339を受け付ける() {
        let app = app();
        let out = call(
            &app,
            "add_schedule",
            json!({
                "text": "RFC3339の予約",
                "at": "2030-06-20T18:00:00+09:00",
                "sns": "bluesky",
            }),
        )
        .await
        .unwrap();

        assert!(out.contains("Successfully scheduled post"));
        let posts = app.state.store.get_all_posts().await.unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].content, "RFC3339の予約");
        assert_eq!(posts[0].target_sns, vec!["bluesky".to_string()]);
    }

    /// 秒まで含む日時形式を受け付ける。
    #[tokio::test]
    async fn add_schedule_は秒付きの日時を受け付ける() {
        let app = app();
        call(
            &app,
            "add_schedule",
            json!({ "text": "秒あり", "at": "2030-06-20 18:00:30", "sns": "bluesky" }),
        )
        .await
        .unwrap();

        let posts = app.state.store.get_all_posts().await.unwrap();
        assert_eq!(
            posts[0].scheduled_at.format("%H:%M:%S").to_string(),
            "18:00:30"
        );
    }

    /// 秒を省いた日時形式も受け付ける。
    #[tokio::test]
    async fn add_schedule_は秒無しの日時を受け付ける() {
        let app = app();
        call(
            &app,
            "add_schedule",
            json!({ "text": "秒なし", "at": "2030-06-20 18:00", "sns": "bluesky" }),
        )
        .await
        .unwrap();

        let posts = app.state.store.get_all_posts().await.unwrap();
        assert_eq!(
            posts[0].scheduled_at.format("%H:%M:%S").to_string(),
            "18:00:00"
        );
    }

    /// 解釈できない日時形式はエラーになる。
    #[tokio::test]
    async fn add_schedule_は不正な日時を拒否する() {
        let app = app();
        let err = call(
            &app,
            "add_schedule",
            json!({ "text": "本文", "at": "きのう", "sns": "bluesky" }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("Invalid datetime format"));
    }

    /// sns はカンマ区切りで複数指定できる。
    #[tokio::test]
    async fn add_schedule_はカンマ区切りのsnsを分解する() {
        let app = app();
        call(
            &app,
            "add_schedule",
            json!({
                "text": "複数SNS",
                "at": "2030-06-20 18:00",
                "sns": " bluesky , mstdn-main ,",
            }),
        )
        .await
        .unwrap();

        let posts = app.state.store.get_all_posts().await.unwrap();
        // 空要素は取り除き、前後の空白も落とす
        assert_eq!(
            posts[0].target_sns,
            vec!["bluesky".to_string(), "mstdn-main".to_string()]
        );
    }

    /// sns 未指定なら設定済みの全 SNS が対象になる。
    #[tokio::test]
    async fn add_schedule_はsns未指定なら設定の全件を使う() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.sns = vec![
                SnsConfig::Mastodon {
                    name: "mstdn-main".to_string(),
                    instance_url: "https://mstdn.example.com".to_string(),
                    access_token: "t".to_string(),
                },
                SnsConfig::Bluesky {
                    name: "bsky-main".to_string(),
                    identifier: "user.example.com".to_string(),
                    password: "p".to_string(),
                },
            ];
        });

        call(
            &app,
            "add_schedule",
            json!({ "text": "全SNS", "at": "2030-06-20 18:00" }),
        )
        .await
        .unwrap();

        let posts = app.state.store.get_all_posts().await.unwrap();
        assert_eq!(
            posts[0].target_sns,
            vec!["mstdn-main".to_string(), "bsky-main".to_string()]
        );
    }

    /// link を渡すと予約に保存される。
    #[tokio::test]
    async fn add_schedule_はlinkを保存する() {
        let app = app();
        call(
            &app,
            "add_schedule",
            json!({
                "text": "リンク付き",
                "at": "2030-06-20 18:00",
                "sns": "bluesky",
                "link": "https://blog.example.com/entry/1",
            }),
        )
        .await
        .unwrap();

        let posts = app.state.store.get_all_posts().await.unwrap();
        assert_eq!(
            posts[0].link_url.as_deref(),
            Some("https://blog.example.com/entry/1")
        );
    }

    /// auto_slot は SNS ごとに空き枠を探して個別の予約を作る。
    #[tokio::test]
    async fn add_schedule_はauto_slotでsnsごとに予約を作る() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.default_allowed_timings = Some(vec![(
                "*".to_string(),
                vec!["09:00".to_string(), "18:00".to_string()],
            )]);
            config.sns = vec![
                SnsConfig::Mastodon {
                    name: "mstdn-main".to_string(),
                    instance_url: "https://mstdn.example.com".to_string(),
                    access_token: "t".to_string(),
                },
                SnsConfig::Bluesky {
                    name: "bsky-main".to_string(),
                    identifier: "user.example.com".to_string(),
                    password: "p".to_string(),
                },
            ];
        });

        let out = call(
            &app,
            "add_schedule",
            json!({ "text": "自動枠", "auto_slot": true }),
        )
        .await
        .unwrap();

        assert!(out.contains("via auto-slot"));

        let posts = app.state.store.get_all_posts().await.unwrap();
        assert_eq!(posts.len(), 2, "SNSごとに1件ずつ作られる");
        for p in &posts {
            assert_eq!(p.target_sns.len(), 1, "1件の予約は1SNSのみを対象にする");
            let hhmm = p.scheduled_at.format("%H:%M").to_string();
            assert!(
                hhmm == "09:00" || hhmm == "18:00",
                "設定した枠であること: {}",
                hhmm
            );
        }
    }

    // --- update_schedule ---

    /// id が無ければエラーになる。
    #[tokio::test]
    async fn update_schedule_はid必須() {
        let app = app();
        let err = call(&app, "update_schedule", json!({ "text": "x" }))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("id is required"));
    }

    /// 存在しない id はエラーになる。
    #[tokio::test]
    async fn update_schedule_は存在しないidを拒否する() {
        let app = app();
        let err = call(&app, "update_schedule", json!({ "id": "no-such-id" }))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("not found"));
    }

    /// text だけを渡すと他の項目は変わらない。
    #[tokio::test]
    async fn update_schedule_はtextのみ部分更新できる() {
        let app = app();
        let id = seed(&app, "元の本文", 60).await;
        let before = app.state.store.get_post_by_id(&id).await.unwrap().unwrap();

        call(
            &app,
            "update_schedule",
            json!({ "id": id, "text": "新しい本文" }),
        )
        .await
        .unwrap();

        let after = app.state.store.get_post_by_id(&id).await.unwrap().unwrap();
        assert_eq!(after.content, "新しい本文");
        assert_eq!(after.scheduled_at, before.scheduled_at, "時刻は変わらない");
        assert_eq!(after.target_sns, before.target_sns, "SNSは変わらない");
    }

    /// at だけを渡すと本文は変わらない。
    #[tokio::test]
    async fn update_schedule_はatのみ部分更新できる() {
        let app = app();
        let id = seed(&app, "本文はそのまま", 60).await;

        call(
            &app,
            "update_schedule",
            json!({ "id": id, "at": "2030-06-20 09:00" }),
        )
        .await
        .unwrap();

        let after = app.state.store.get_post_by_id(&id).await.unwrap().unwrap();
        assert_eq!(after.content, "本文はそのまま");
        assert_eq!(
            after.scheduled_at.format("%Y-%m-%d %H:%M").to_string(),
            "2030-06-20 09:00"
        );
    }

    /// status と link と sns も更新できる。
    #[tokio::test]
    async fn update_schedule_はstatusとlinkとsnsを更新する() {
        let app = app();
        let id = seed(&app, "更新対象", 60).await;

        call(
            &app,
            "update_schedule",
            json!({
                "id": id,
                "status": "失敗",
                "link": "https://blog.example.com/entry/2",
                "sns": "mstdn-main, bsky-main",
            }),
        )
        .await
        .unwrap();

        let after = app.state.store.get_post_by_id(&id).await.unwrap().unwrap();
        assert_eq!(after.status, "失敗");
        assert_eq!(
            after.link_url.as_deref(),
            Some("https://blog.example.com/entry/2")
        );
        assert_eq!(
            after.target_sns,
            vec!["mstdn-main".to_string(), "bsky-main".to_string()]
        );
    }

    /// 更新すると updated_at が進む。
    #[tokio::test]
    async fn update_schedule_はupdated_atを進める() {
        let app = app();
        let id = seed(&app, "更新時刻の確認", 60).await;
        let before = app.state.store.get_post_by_id(&id).await.unwrap().unwrap();

        call(
            &app,
            "update_schedule",
            json!({ "id": id, "text": "更新後" }),
        )
        .await
        .unwrap();

        let after = app.state.store.get_post_by_id(&id).await.unwrap().unwrap();
        assert!(
            after.updated_at >= before.updated_at,
            "更新時刻が巻き戻らないこと"
        );
    }

    /// 不正な日時は更新を拒否する。
    #[tokio::test]
    async fn update_schedule_は不正な日時を拒否する() {
        let app = app();
        let id = seed(&app, "日時不正", 60).await;

        let err = call(&app, "update_schedule", json!({ "id": id, "at": "あした" }))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("Invalid datetime format"));
    }

    // --- delete_schedule ---

    /// id が無ければエラーになる。
    #[tokio::test]
    async fn delete_schedule_はid必須() {
        let app = app();
        let err = call(&app, "delete_schedule", json!({})).await.unwrap_err();

        assert!(err.to_string().contains("id is required"));
    }

    /// 削除に成功すると保存先からも消える。
    #[tokio::test]
    async fn delete_schedule_は予約を削除する() {
        let app = app();
        let id = seed(&app, "消す予約", 60).await;

        let out = call(&app, "delete_schedule", json!({ "id": id }))
            .await
            .unwrap();

        assert!(out.contains("Successfully deleted"));
        assert!(app.state.store.get_post_by_id(&id).await.unwrap().is_none());
    }

    /// 存在しない id はエラーになる。
    #[tokio::test]
    async fn delete_schedule_は存在しないidを拒否する() {
        let app = app();
        let err = call(&app, "delete_schedule", json!({ "id": "no-such-id" }))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("not found"));
    }

    // --- post_now ---

    /// text が無ければエラーになる。
    #[tokio::test]
    async fn post_now_はtext必須() {
        let app = app();
        let err = call(&app, "post_now", json!({})).await.unwrap_err();

        assert!(err.to_string().contains("text is required"));
    }

    /// 対象のクライアントが1つも無ければエラーになる。
    #[tokio::test]
    async fn post_now_は対象クライアントが無ければエラー() {
        let app = app();
        let err = call(&app, "post_now", json!({ "text": "宛先なし" }))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("No active SNS client"));
    }

    /// モックサーバを instance_url に差し込んだ Mastodon 設定を作る。
    fn app_with_mock_mastodon(server: &wiremock::MockServer) -> TestApp {
        let uri = server.uri();
        setup_test_app_with_config(Some(SECRET.to_string()), move |config| {
            config.sns = vec![SnsConfig::Mastodon {
                name: "mstdn-main".to_string(),
                instance_url: uri,
                access_token: "t".to_string(),
            }];
        })
    }

    /// 投稿が成功すると Success として結果に載る。
    #[tokio::test]
    async fn post_now_は成功結果を返す() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "url": "https://mstdn.example.com/@u/1" })),
            )
            .mount(&server)
            .await;

        let app = app_with_mock_mastodon(&server);
        let out = call(&app, "post_now", json!({ "text": "即時投稿" }))
            .await
            .unwrap();

        assert!(out.contains("投稿実行結果"));
        // 出力に載るのは SnsClient::name() つまり種別名であり、アカウント名ではない
        assert!(out.contains("Posting to mastodon"), "実際の出力:\n{}", out);
        assert!(out.contains("[Success]"), "実際の出力:\n{}", out);
        assert!(out.contains("https://mstdn.example.com/@u/1"));
    }

    /// 投稿先がエラーを返すと Failed か Error として結果に載る。
    #[tokio::test]
    async fn post_now_は失敗を結果に載せる() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let app = app_with_mock_mastodon(&server);
        let out = call(&app, "post_now", json!({ "text": "失敗する投稿" }))
            .await
            .unwrap();

        assert!(
            out.contains("[Failed]") || out.contains("[Error]"),
            "失敗が記録されること:\n{}",
            out
        );
    }

    /// sns 引数でアカウント名を指定すると、その1件だけが対象になる。
    #[tokio::test]
    async fn post_now_はsns名で絞り込む() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "url": "https://mstdn.example.com/@u/2" })),
            )
            .mount(&server)
            .await;

        let uri = server.uri();
        let app = setup_test_app_with_config(Some(SECRET.to_string()), move |config| {
            config.sns = vec![
                SnsConfig::Mastodon {
                    name: "mstdn-main".to_string(),
                    instance_url: uri,
                    access_token: "t".to_string(),
                },
                SnsConfig::Bluesky {
                    name: "bsky-main".to_string(),
                    identifier: "user.example.com".to_string(),
                    password: "p".to_string(),
                },
            ];
        });

        let out = call(
            &app,
            "post_now",
            json!({ "text": "絞り込み投稿", "sns": "mstdn-main" }),
        )
        .await
        .unwrap();

        // 出力は種別名で載るため、対象が1件に絞られたことを件数で確かめる
        assert_eq!(
            out.matches("Posting to ").count(),
            1,
            "指定した1件だけが対象になること:\n{}",
            out
        );
        assert!(out.contains("Posting to mastodon"), "実際の出力:\n{}", out);
        assert!(!out.contains("bluesky"), "指定外は対象にしない:\n{}", out);
    }

    /// sns 引数には種別名も使える。
    #[tokio::test]
    async fn post_now_は種別名でも絞り込める() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "url": "https://mstdn.example.com/@u/3" })),
            )
            .mount(&server)
            .await;

        let app = app_with_mock_mastodon(&server);
        let out = call(
            &app,
            "post_now",
            json!({ "text": "種別指定", "sns": "mastodon" }),
        )
        .await
        .unwrap();

        assert!(out.contains("[Success]"), "実際の出力:\n{}", out);
    }

    /// 一致するクライアントが無い絞り込みはエラーになる。
    #[tokio::test]
    async fn post_now_は一致しない絞り込みをエラーにする() {
        use wiremock::MockServer;

        let server = MockServer::start().await;
        let app = app_with_mock_mastodon(&server);

        let err = call(
            &app,
            "post_now",
            json!({ "text": "宛先違い", "sns": "no-such-sns" }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("No active SNS client"));
    }

    // --- 未知のtool ---

    // --- media 引数の検証 ---

    /// アップロード領域に検証用の画像を1枚置き、そのパスを返す。
    fn seed_upload_image(app: &TestApp, name: &str) -> std::path::PathBuf {
        std::fs::create_dir_all(&app.state.upload_dir).expect("アップロード領域の作成に失敗");
        let path = app.state.upload_dir.join(name);

        let mut buf = std::io::Cursor::new(Vec::new());
        image::RgbImage::new(1, 1)
            .write_to(&mut buf, image::ImageFormat::Png)
            .expect("PNGの書き出しに失敗");
        std::fs::write(&path, buf.into_inner()).expect("画像の書き込みに失敗");

        path
    }

    /// 許可ディレクトリの外にあるファイルを作る。
    fn seed_outside_file(app: &TestApp, name: &str, content: &[u8]) -> std::path::PathBuf {
        let parent = app
            .state
            .upload_dir
            .parent()
            .expect("アップロード領域には親がある")
            .to_path_buf();
        let path = parent.join(name);
        std::fs::write(&path, content).expect("ファイルの書き込みに失敗");
        path
    }

    /// アップロード領域の画像は予約に添付できる。
    #[tokio::test]
    async fn add_scheduleは許可領域のメディアを受け付ける() {
        let app = app();
        let image = seed_upload_image(&app, "ok.png");

        call(
            &app,
            "add_schedule",
            json!({
                "text": "画像つき予約",
                "at": "2030-06-20 18:00",
                "sns": "bluesky",
                "media": [image.to_str().unwrap()],
            }),
        )
        .await
        .unwrap();

        let posts = app.state.store.get_all_posts().await.unwrap();
        assert_eq!(posts[0].media_files.len(), 1);

        // 元ファイルとは別に、アップロード領域へ複製される
        let saved = std::path::Path::new(&posts[0].media_files[0]);
        assert!(saved.exists(), "複製先が存在すること");
        assert_ne!(saved, image, "元ファイルをそのまま参照していない");
        assert!(saved.starts_with(&app.state.upload_dir));
    }

    /// 許可ディレクトリの外は予約に添付できない。
    #[tokio::test]
    async fn add_scheduleは許可領域外のメディアを拒否する() {
        let app = app();
        // 秘密ファイルを模す。実際に読み出せてしまうと SNS へ送信される
        let outside = seed_outside_file(&app, "id_rsa", b"PRIVATE KEY");

        let err = call(
            &app,
            "add_schedule",
            json!({
                "text": "情報を持ち出す予約",
                "at": "2030-06-20 18:00",
                "sns": "bluesky",
                "media": [outside.to_str().unwrap()],
            }),
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string().contains("outside the allowed directories"),
            "実際のエラー: {}",
            err
        );
        assert!(
            app.state.store.get_all_posts().await.unwrap().is_empty(),
            "拒否したら予約も作らない"
        );
    }

    /// 即時投稿でも許可ディレクトリの外は拒否する。
    #[tokio::test]
    async fn post_nowは許可領域外のメディアを拒否する() {
        use wiremock::MockServer;

        let server = MockServer::start().await;
        let uri = server.uri();
        let app = setup_test_app_with_config(Some(SECRET.to_string()), move |config| {
            config.sns = vec![SnsConfig::Mastodon {
                name: "mstdn-main".to_string(),
                instance_url: uri,
                access_token: "t".to_string(),
            }];
        });
        let outside = seed_outside_file(&app, "id_rsa", b"PRIVATE KEY");

        let err = call(
            &app,
            "post_now",
            json!({
                "text": "情報を持ち出す投稿",
                "media": [outside.to_str().unwrap()],
            }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("outside the allowed directories"));
        // モックへは1件も届いていない
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    /// 存在しないファイルは拒否する。
    #[tokio::test]
    async fn 存在しないメディアは拒否する() {
        let app = app();
        let missing = app.state.upload_dir.join("nope.png");

        let err = call(
            &app,
            "add_schedule",
            json!({
                "text": "無いファイル",
                "at": "2030-06-20 18:00",
                "sns": "bluesky",
                "media": [missing.to_str().unwrap()],
            }),
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string().contains("not found"),
            "実際のエラー: {}",
            err
        );
    }

    /// 中身が画像でないファイルは拒否する。
    #[tokio::test]
    async fn 画像でないメディアは拒否する() {
        let app = app();
        std::fs::create_dir_all(&app.state.upload_dir).unwrap();
        let fake = app.state.upload_dir.join("fake.png");
        // 拡張子を偽っても中身で弾く
        std::fs::write(&fake, b"not an image at all").unwrap();

        let err = call(
            &app,
            "add_schedule",
            json!({
                "text": "偽装ファイル",
                "at": "2030-06-20 18:00",
                "sns": "bluesky",
                "media": [fake.to_str().unwrap()],
            }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("Unsupported media format"));
    }

    /// media を渡さない場合は何も添付されない。
    #[tokio::test]
    async fn media未指定なら添付なしで予約できる() {
        let app = app();

        call(
            &app,
            "add_schedule",
            json!({ "text": "添付なし", "at": "2030-06-20 18:00", "sns": "bluesky" }),
        )
        .await
        .unwrap();

        let posts = app.state.store.get_all_posts().await.unwrap();
        assert!(posts[0].media_files.is_empty());
    }

    // --- sns 引数の分解 ---

    #[test]
    fn sns引数を分解できる() {
        assert_eq!(
            parse_sns_targets(Some(" bluesky , mstdn-main ,")),
            Some(vec!["bluesky".to_string(), "mstdn-main".to_string()])
        );
    }

    #[test]
    fn sns引数が未指定や空ならnoneになる() {
        assert_eq!(parse_sns_targets(None), None);
        assert_eq!(parse_sns_targets(Some("")), None);
        // 区切りだけの指定も未指定と同じ扱いにする
        assert_eq!(parse_sns_targets(Some(" , , ")), None);
    }

    // --- sensitive フラグ ---

    /// sensitive を渡すと予約に保存される。
    #[tokio::test]
    async fn add_scheduleはsensitiveを保存する() {
        let app = app();

        call(
            &app,
            "add_schedule",
            json!({
                "text": "センシティブ指定",
                "at": "2030-06-20 18:00",
                "sns": "bluesky",
                "sensitive": true,
            }),
        )
        .await
        .unwrap();

        let posts = app.state.store.get_all_posts().await.unwrap();
        assert!(posts[0].sensitive, "sensitive が予約へ伝わること");
    }

    /// sensitive 未指定なら false になる。
    #[tokio::test]
    async fn add_scheduleはsensitive未指定でfalseになる() {
        let app = app();

        call(
            &app,
            "add_schedule",
            json!({ "text": "通常投稿", "at": "2030-06-20 18:00", "sns": "bluesky" }),
        )
        .await
        .unwrap();

        let posts = app.state.store.get_all_posts().await.unwrap();
        assert!(!posts[0].sensitive);
    }

    /// auto_slot で作った予約にも sensitive が伝わる。
    #[tokio::test]
    async fn auto_slotの予約にもsensitiveが伝わる() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.default_allowed_timings =
                Some(vec![("*".to_string(), vec!["09:00".to_string()])]);
            config.sns = vec![SnsConfig::Bluesky {
                name: "bsky-main".to_string(),
                identifier: "user.example.com".to_string(),
                password: "p".to_string(),
            }];
        });

        call(
            &app,
            "add_schedule",
            json!({ "text": "自動枠", "auto_slot": true, "sensitive": true }),
        )
        .await
        .unwrap();

        let posts = app.state.store.get_all_posts().await.unwrap();
        assert!(posts[0].sensitive);
    }

    // --- 未対応SNSの明示エラー ---

    /// Threads を指定すると未対応であることを明示する。
    #[tokio::test]
    async fn post_nowは未対応snsを明示エラーにする() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.sns = vec![SnsConfig::Threads {
                name: "threads-main".to_string(),
                user_id: "1".to_string(),
                access_token: "t".to_string(),
            }];
        });

        let err = call(
            &app,
            "post_now",
            json!({ "text": "本文", "sns": "threads-main" }),
        )
        .await
        .unwrap_err();

        // 従来は黙ってスキップされ「宛先が無い」としか分からなかった
        assert!(
            err.to_string().contains("posting is not implemented"),
            "実際のエラー: {}",
            err
        );
        assert!(err.to_string().contains("threads-main"));
    }

    /// Tumblr も同様に明示エラーになる。
    #[tokio::test]
    async fn post_nowはtumblrも明示エラーにする() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.sns = vec![SnsConfig::Tumblr {
                name: "tumblr-main".to_string(),
                consumer_key: "k".to_string(),
                consumer_secret: "s".to_string(),
                oauth_token: "t".to_string(),
                oauth_secret: "ts".to_string(),
                blog_identifier: "blog.example.com".to_string(),
            }];
        });

        let err = call(&app, "post_now", json!({ "text": "本文", "sns": "tumblr" }))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("posting is not implemented"));
    }

    /// sns 未指定なら未対応の種別は対象にせず、対応分だけ投稿する。
    #[tokio::test]
    async fn sns未指定なら未対応種別は対象にしない() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "url": "https://mstdn.example.com/@u/9" })),
            )
            .mount(&server)
            .await;

        let uri = server.uri();
        let app = setup_test_app_with_config(Some(SECRET.to_string()), move |config| {
            config.sns = vec![
                SnsConfig::Mastodon {
                    name: "mstdn-main".to_string(),
                    instance_url: uri,
                    access_token: "t".to_string(),
                },
                SnsConfig::Threads {
                    name: "threads-main".to_string(),
                    user_id: "1".to_string(),
                    access_token: "t".to_string(),
                },
            ];
        });

        // 全件宛ての投稿が未対応種別のせいで失敗しては困る
        let out = call(&app, "post_now", json!({ "text": "全件投稿" }))
            .await
            .unwrap();

        assert!(out.contains("[Success]"), "実際の出力:\n{}", out);
    }

    /// add_schedule も sns 未指定では未対応種別を含めない。
    #[tokio::test]
    async fn add_scheduleはsns未指定で未対応種別を含めない() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.sns = vec![
                SnsConfig::Bluesky {
                    name: "bsky-main".to_string(),
                    identifier: "user.example.com".to_string(),
                    password: "p".to_string(),
                },
                SnsConfig::Threads {
                    name: "threads-main".to_string(),
                    user_id: "1".to_string(),
                    access_token: "t".to_string(),
                },
            ];
        });

        call(
            &app,
            "add_schedule",
            json!({ "text": "全件予約", "at": "2030-06-20 18:00" }),
        )
        .await
        .unwrap();

        let posts = app.state.store.get_all_posts().await.unwrap();
        // 投稿できない先を予約に入れても実行時に失敗するだけ
        assert_eq!(posts[0].target_sns, vec!["bsky-main".to_string()]);
    }

    // --- get_next_slots ---

    /// 設定したタイミングの枠が返る。
    #[tokio::test]
    async fn get_next_slotsは次の枠を返す() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.default_allowed_timings = Some(vec![(
                "*".to_string(),
                vec!["09:00".to_string(), "18:00".to_string()],
            )]);
            config.sns = vec![SnsConfig::Mastodon {
                name: "mstdn-main".to_string(),
                instance_url: "https://mstdn.example.com".to_string(),
                access_token: "t".to_string(),
            }];
        });

        let out = call(&app, "get_next_slots", json!({})).await.unwrap();

        assert!(out.contains("次の投稿枠"));
        assert!(out.contains("mstdn-main"));
        assert!(
            out.contains("09:00:00") || out.contains("18:00:00"),
            "設定したタイミングの枠であること:\n{}",
            out
        );
    }

    /// SNS が未設定なら対象が無い旨を返す。
    #[tokio::test]
    async fn get_next_slotsはsns未設定で案内を返す() {
        let app = app();

        let out = call(&app, "get_next_slots", json!({})).await.unwrap();

        assert!(
            out.contains("(対象のSNSがありません)"),
            "実際の出力:\n{}",
            out
        );
    }

    /// sns 引数で対象を絞れる。
    #[tokio::test]
    async fn get_next_slotsはsnsで絞り込める() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.default_allowed_timings =
                Some(vec![("*".to_string(), vec!["09:00".to_string()])]);
            config.sns = vec![
                SnsConfig::Mastodon {
                    name: "mstdn-main".to_string(),
                    instance_url: "https://mstdn.example.com".to_string(),
                    access_token: "t".to_string(),
                },
                SnsConfig::Bluesky {
                    name: "bsky-main".to_string(),
                    identifier: "user.example.com".to_string(),
                    password: "p".to_string(),
                },
            ];
        });

        let out = call(&app, "get_next_slots", json!({ "sns": "bsky-main" }))
            .await
            .unwrap();

        assert!(out.contains("bsky-main"));
        assert!(!out.contains("mstdn-main"), "実際の出力:\n{}", out);
    }

    /// タイミング未設定なら制限なしとみなし、直近の枠が返る。
    #[tokio::test]
    async fn get_next_slotsはタイミング未設定でも枠を返す() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            // allowed_timings をどこにも設定しない
            config.sns = vec![SnsConfig::Bluesky {
                name: "bsky-main".to_string(),
                identifier: "user.example.com".to_string(),
                password: "p".to_string(),
            }];
        });

        let out = call(&app, "get_next_slots", json!({})).await.unwrap();

        // タイミングを設定していない場合はいつでも投稿できる扱いになる
        assert!(out.contains("bsky-main"));
        assert!(
            !out.contains("空き枠が見つかりません"),
            "実際の出力:\n{}",
            out
        );
    }

    /// 未知の tool 名はエラーになる。
    #[tokio::test]
    async fn 未知のtool名はエラーになる() {
        let app = app();
        let err = call(&app, "no_such_tool", json!({})).await.unwrap_err();

        assert!(err.to_string().contains("Unknown tool name"));
    }
}
