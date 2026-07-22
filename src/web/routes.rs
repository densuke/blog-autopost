use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::AppState;
use crate::config::SnsConfig;
use crate::sns::models::{PostContent, PostResult};
use crate::sns::traits::SnsClient;
use crate::sns::{
    bluesky::BlueskyClient, mastodon::MastodonClient, misskey::MisskeyClient, x::XClient,
};

/// Web UI へ返す SNS アカウント1件分の情報。
///
/// `name` が実際の識別子であり、`label` は表示専用である。
/// 表示文字列からアカウント名を逆算する処理を UI 側に持たせないために
/// 両者を分けて返す。
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct SnsAccountInfo {
    /// config.yml に記載されたアカウント名 (投稿対象の識別子)
    pub name: String,
    /// SNS の種別 (`x` / `bluesky` / `mastodon` / `misskey` など)
    pub sns_type: String,
    /// 画面表示用のラベル (例: `X (x)`)
    pub label: String,
}

#[derive(Serialize)]
pub struct ConfigResponse {
    pub blog_name: String,
    /// 表示用ラベルの一覧 (既存クライアント互換のために維持)
    pub active_sns: Vec<String>,
    /// SNS アカウントの構造化一覧
    pub sns_accounts: Vec<SnsAccountInfo>,
}

/// `SnsConfig` からアカウント名を取り出す。未知種別は `None` を返す。
pub fn sns_account_name(sns: &SnsConfig) -> Option<&str> {
    match sns {
        SnsConfig::Mastodon { name, .. }
        | SnsConfig::Misskey { name, .. }
        | SnsConfig::Bluesky { name, .. }
        | SnsConfig::X { name, .. }
        | SnsConfig::Threads { name, .. }
        | SnsConfig::Tumblr { name, .. } => Some(name),
        SnsConfig::Unknown => None,
    }
}

/// `SnsConfig` から種別名を取り出す。未知種別は `None` を返す。
pub fn sns_type_name(sns: &SnsConfig) -> Option<&'static str> {
    match sns {
        SnsConfig::Mastodon { .. } => Some("mastodon"),
        SnsConfig::Misskey { .. } => Some("misskey"),
        SnsConfig::Bluesky { .. } => Some("bluesky"),
        SnsConfig::X { .. } => Some("x"),
        SnsConfig::Threads { .. } => Some("threads"),
        SnsConfig::Tumblr { .. } => Some("tumblr"),
        SnsConfig::Unknown => None,
    }
}

/// 画面表示用のラベル (例: `X (x)`) を生成する。未知種別は `None` を返す。
pub fn sns_display_label(sns: &SnsConfig) -> Option<String> {
    let name = sns_account_name(sns)?;
    let type_label = match sns {
        SnsConfig::Mastodon { .. } => "Mastodon",
        SnsConfig::Misskey { .. } => "Misskey",
        SnsConfig::Bluesky { .. } => "Bluesky",
        SnsConfig::X { .. } => "X",
        SnsConfig::Threads { .. } => "Threads",
        SnsConfig::Tumblr { .. } => "Tumblr",
        SnsConfig::Unknown => return None,
    };
    Some(format!("{} ({})", type_label, name))
}

/// 設定から Web UI 用の SNS アカウント一覧を組み立てる。未知種別は除外する。
pub fn build_sns_accounts(sns_list: &[SnsConfig]) -> Vec<SnsAccountInfo> {
    sns_list
        .iter()
        .filter_map(|s| {
            Some(SnsAccountInfo {
                name: sns_account_name(s)?.to_string(),
                sns_type: sns_type_name(s)?.to_string(),
                label: sns_display_label(s)?,
            })
        })
        .collect()
}

/// 投稿対象の指定文字列からアカウント名を解決する。
///
/// アカウント名そのものと表示用ラベルの双方を受け付ける。
/// 表示用ラベルを正規表現で分解する必要がないため、
/// アカウント名に括弧が含まれていても正しく解決できる。
pub fn resolve_sns_name(sns_list: &[SnsConfig], target: &str) -> Option<String> {
    sns_list.iter().find_map(|s| {
        let name = sns_account_name(s)?;
        if name == target || sns_display_label(s).as_deref() == Some(target) {
            Some(name.to_string())
        } else {
            None
        }
    })
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> Json<ConfigResponse> {
    let blog_name = state
        .config
        .blog
        .as_ref()
        .and_then(|b| b.first())
        .map(|b| b.name.clone())
        .unwrap_or_else(|| "Unknown Blog".to_string());

    let sns_accounts = build_sns_accounts(&state.config.sns);
    let active_sns = sns_accounts.iter().map(|a| a.label.clone()).collect();

    Json(ConfigResponse {
        blog_name,
        active_sns,
        sns_accounts,
    })
}

#[derive(Deserialize)]
pub struct ManualPostRequest {
    pub text: String,
    pub image_url: Option<String>,
    pub media_paths: Option<Vec<String>>,
    pub link_url: Option<String>,
    pub targets: Option<Vec<String>>,
    pub schedule_type: Option<String>,
    pub scheduled_at: Option<String>,
    /// 添付メディアをセンシティブコンテンツとして扱うか（現状 Misskey のみ対応）
    #[serde(default)]
    pub sensitive: Option<bool>,
}

#[derive(Serialize)]
pub struct ManualPostResponse {
    pub success: bool,
    pub results: Vec<PostResult>,
}

pub async fn manual_post(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ManualPostRequest>,
) -> Json<ManualPostResponse> {
    // リクエストごとに SnsClient を組み立てる (KISS実装)
    let mut sns_clients: Vec<Box<dyn SnsClient + Send + Sync>> = Vec::new();
    for sns_conf in &state.config.sns {
        let Some(account_name) = sns_account_name(sns_conf) else {
            continue;
        };
        let Some(target_label) = sns_display_label(sns_conf) else {
            continue;
        };

        // アカウント名と表示用ラベルのどちらで指定されても受け付ける
        if let Some(ref selected) = payload.targets
            && !selected
                .iter()
                .any(|t| t == account_name || *t == target_label)
        {
            continue;
        }

        match sns_conf {
            SnsConfig::Mastodon {
                instance_url,
                access_token,
                name,
                ..
            } => {
                if let Ok(client) =
                    MastodonClient::new(instance_url.clone(), access_token.clone(), name.clone())
                {
                    sns_clients.push(Box::new(client));
                }
            }
            SnsConfig::Misskey {
                instance_url,
                access_token,
                name,
                ..
            } => {
                if let Ok(client) =
                    MisskeyClient::new(instance_url.clone(), access_token.clone(), name.clone())
                {
                    sns_clients.push(Box::new(client));
                }
            }
            SnsConfig::Bluesky {
                identifier,
                password,
                name,
                ..
            } => {
                if let Ok(client) =
                    BlueskyClient::new(identifier.clone(), password.clone(), name.clone())
                {
                    sns_clients.push(Box::new(client));
                }
            }
            SnsConfig::X {
                consumer_key,
                consumer_secret,
                access_token,
                access_token_secret,
                name,
            } => {
                if let Ok(client) = XClient::new(
                    consumer_key.clone(),
                    consumer_secret.clone(),
                    access_token.clone(),
                    access_token_secret.clone(),
                    name.clone(),
                ) {
                    sns_clients.push(Box::new(client));
                }
            }
            _ => {}
        }
    }

    let schedule_type = payload
        .schedule_type
        .clone()
        .unwrap_or_else(|| "now".to_string());

    if schedule_type == "now" {
        let post_content = PostContent {
            text: payload.text,
            image_url: payload.image_url,
            media_paths: payload.media_paths,
            link_url: payload.link_url,
            sensitive: payload.sensitive.unwrap_or(false),
        };

        let mut results = Vec::new();
        let mut all_success = true;

        for client in sns_clients {
            match client.post(&post_content).await {
                Ok(result) => {
                    if !result.success {
                        all_success = false;
                    }
                    results.push(result);
                }
                Err(e) => {
                    all_success = false;
                    results.push(PostResult {
                        success: false,
                        post_id: None,
                        error_message: Some(e.to_string()),
                    });
                }
            }
        }

        Json(ManualPostResponse {
            success: all_success,
            results,
        })
    } else {
        use crate::scheduled::ScheduledPost;
        use crate::timing::SlotFinder;

        let targets = payload.targets.clone().unwrap_or_default();
        if targets.is_empty() {
            return Json(ManualPostResponse {
                success: false,
                results: vec![PostResult {
                    success: false,
                    post_id: None,
                    error_message: Some("No target SNS selected for scheduling".to_string()),
                }],
            });
        }

        let finder = SlotFinder::new(&state.timing_manager, &state.store, 5);
        let mut results = Vec::new();
        let mut all_success = true;

        for target in &targets {
            let sns_name = resolve_sns_name(&state.config.sns, target);

            let Some(sns_name) = sns_name else {
                all_success = false;
                results.push(PostResult {
                    success: false,
                    post_id: None,
                    error_message: Some(format!("Unknown SNS target: {}", target)),
                });
                continue;
            };

            let scheduled_time = if schedule_type == "next" {
                match finder.find_next_available_slot(&sns_name, None, 7).await {
                    Ok(Some(dt)) => dt,
                    Ok(None) => {
                        all_success = false;
                        results.push(PostResult {
                            success: false,
                            post_id: None,
                            error_message: Some(format!("No available slot found for {}", target)),
                        });
                        continue;
                    }
                    Err(e) => {
                        all_success = false;
                        results.push(PostResult {
                            success: false,
                            post_id: None,
                            error_message: Some(format!(
                                "Failed to calculate slot for {}: {}",
                                target, e
                            )),
                        });
                        continue;
                    }
                }
            } else {
                let Some(at_str) = &payload.scheduled_at else {
                    all_success = false;
                    results.push(PostResult {
                        success: false,
                        post_id: None,
                        error_message: Some(
                            "Missing scheduled_at time for custom schedule".to_string(),
                        ),
                    });
                    continue;
                };
                match chrono::DateTime::parse_from_rfc3339(at_str) {
                    Ok(dt) => dt.with_timezone(&chrono::Local),
                    Err(e) => {
                        all_success = false;
                        results.push(PostResult {
                            success: false,
                            post_id: None,
                            error_message: Some(format!("Invalid custom datetime format: {}", e)),
                        });
                        continue;
                    }
                }
            };

            let mut media_files = payload.media_paths.clone().unwrap_or_default();
            if media_files.is_empty()
                && let Some(img_url) = &payload.image_url
            {
                media_files.push(img_url.clone());
            }
            let mut post = ScheduledPost::new(
                payload.text.clone(),
                scheduled_time,
                media_files,
                vec![sns_name.clone()],
            );
            post.link_url = payload.link_url.clone();
            post.sensitive = payload.sensitive.unwrap_or(false);

            match state.store.create_post(post).await {
                Ok(_) => {
                    results.push(PostResult {
                        success: true,
                        post_id: Some(format!("scheduled at {}", scheduled_time.to_rfc3339())),
                        error_message: None,
                    });
                }
                Err(e) => {
                    all_success = false;
                    results.push(PostResult {
                        success: false,
                        post_id: None,
                        error_message: Some(format!("Failed to save schedule: {}", e)),
                    });
                }
            }
        }

        Json(ManualPostResponse {
            success: all_success,
            results,
        })
    }
}

#[derive(Serialize)]
pub struct NextSlotResponse {
    pub slots: HashMap<String, Option<String>>,
}

pub async fn get_next_slots(
    State(state): State<Arc<AppState>>,
) -> Result<Json<NextSlotResponse>, StatusCode> {
    use crate::timing::SlotFinder;

    let finder = SlotFinder::new(&state.timing_manager, &state.store, 5);
    let mut slots = HashMap::new();

    for sns_conf in &state.config.sns {
        let Some(name) = sns_account_name(sns_conf) else {
            continue;
        };

        let slot = finder
            .find_next_available_slot(name, None, 7)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        slots.insert(name.to_string(), slot.map(|dt| dt.to_rfc3339()));
    }

    Ok(Json(NextSlotResponse { slots }))
}

// GET /api/schedules
pub async fn get_schedules(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::scheduled::ScheduledPost>>, StatusCode> {
    state.store.get_all_posts().await.map(Json).map_err(|e| {
        println!("Failed to get schedules: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

#[derive(Deserialize)]
pub struct UpdateScheduleRequest {
    pub content: String,
    pub scheduled_at: String,
    pub target_sns: Vec<String>,
    pub status: String,
    pub media_files: Option<Vec<String>>,
    pub link_url: Option<String>,
}

// PUT /api/schedules/:id
pub async fn update_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateScheduleRequest>,
) -> Result<Json<crate::scheduled::ScheduledPost>, StatusCode> {
    let scheduled_time = match chrono::DateTime::parse_from_rfc3339(&payload.scheduled_at) {
        Ok(dt) => dt.with_timezone(&chrono::Local),
        Err(e) => {
            println!("Invalid datetime format {}: {:?}", payload.scheduled_at, e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    let existing = state.store.get_post_by_id(&id).await.map_err(|e| {
        println!("Failed to find schedule {}: {:?}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let Some(mut post) = existing else {
        return Err(StatusCode::NOT_FOUND);
    };

    post.content = payload.content;
    post.scheduled_at = scheduled_time;
    post.target_sns = payload.target_sns;
    post.status = payload.status;
    if let Some(media) = payload.media_files {
        post.media_files = media;
    }
    post.link_url = payload.link_url;
    post.updated_at = chrono::Local::now();

    let updated = state.store.update_post(&id, post).await.map_err(|e| {
        println!("Failed to update schedule {}: {:?}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match updated {
        Some(p) => Ok(Json(p)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// DELETE /api/schedules/:id
pub async fn delete_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let success = state.store.delete_post(&id).await.map_err(|e| {
        println!("Failed to delete schedule {}: {:?}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if success {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Serialize)]
pub struct UploadResponse {
    pub success: bool,
    pub paths: Vec<String>,
    pub error: Option<String>,
}

pub async fn upload_media(mut multipart: Multipart) -> Result<Json<UploadResponse>, StatusCode> {
    let mut saved_paths = Vec::new();

    if let Err(e) = std::fs::create_dir_all("data/uploads") {
        println!("Failed to create upload dir: {:?}", e);
        return Ok(Json(UploadResponse {
            success: false,
            paths: Vec::new(),
            error: Some(format!("Server internal error: {}", e)),
        }));
    }

    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = field.file_name().unwrap_or("file.png").to_string();
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        let allowed_types = [
            "image/jpeg",
            "image/png",
            "image/gif",
            "image/webp",
            "video/mp4",
            "video/quicktime",
        ];

        let mime_base = content_type
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_lowercase();
        if !allowed_types.contains(&mime_base.as_str()) {
            return Ok(Json(UploadResponse {
                success: false,
                paths: Vec::new(),
                error: Some(format!(
                    "許可されていないファイル形式です: {}。許可形式: {}",
                    mime_base,
                    allowed_types.join(", ")
                )),
            }));
        }

        let bytes = match field.bytes().await {
            Ok(b) => b,
            Err(e) => {
                return Ok(Json(UploadResponse {
                    success: false,
                    paths: Vec::new(),
                    error: Some(format!("Failed to read file bytes: {}", e)),
                }));
            }
        };

        let max_size = 10 * 1024 * 1024;
        if bytes.len() > max_size {
            return Ok(Json(UploadResponse {
                success: false,
                paths: Vec::new(),
                error: Some(format!(
                    "ファイルサイズが上限（10MB）を超えています: {} bytes",
                    bytes.len()
                )),
            }));
        }

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

        if let Err(e) = std::fs::write(&save_path, &bytes) {
            println!("Failed to write file to {}: {:?}", save_path, e);
            return Ok(Json(UploadResponse {
                success: false,
                paths: Vec::new(),
                error: Some(format!("Failed to save file: {}", e)),
            }));
        }

        saved_paths.push(save_path);
    }

    Ok(Json(UploadResponse {
        success: true,
        paths: saved_paths,
        error: None,
    }))
}

// POST /api/schedules/:id/post-now
pub async fn post_now_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ManualPostResponse>, StatusCode> {
    let existing = state.store.get_post_by_id(&id).await.map_err(|e| {
        println!("Failed to find schedule {}: {:?}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let Some(mut post) = existing else {
        return Err(StatusCode::NOT_FOUND);
    };

    let mut sns_clients: Vec<Box<dyn SnsClient + Send + Sync>> = Vec::new();
    for sns_conf in &state.config.sns {
        let name = match sns_conf {
            SnsConfig::Mastodon { name, .. } => name,
            SnsConfig::Misskey { name, .. } => name,
            SnsConfig::Bluesky { name, .. } => name,
            SnsConfig::X { name, .. } => name,
            _ => continue,
        };

        if post.target_sns.contains(name) {
            match sns_conf {
                SnsConfig::Mastodon {
                    instance_url,
                    access_token,
                    name,
                    ..
                } => {
                    if let Ok(client) = MastodonClient::new(
                        instance_url.clone(),
                        access_token.clone(),
                        name.clone(),
                    ) {
                        sns_clients.push(Box::new(client));
                    }
                }
                SnsConfig::Misskey {
                    instance_url,
                    access_token,
                    name,
                    ..
                } => {
                    if let Ok(client) =
                        MisskeyClient::new(instance_url.clone(), access_token.clone(), name.clone())
                    {
                        sns_clients.push(Box::new(client));
                    }
                }
                SnsConfig::Bluesky {
                    identifier,
                    password,
                    name,
                    ..
                } => {
                    if let Ok(client) =
                        BlueskyClient::new(identifier.clone(), password.clone(), name.clone())
                    {
                        sns_clients.push(Box::new(client));
                    }
                }
                SnsConfig::X {
                    consumer_key,
                    consumer_secret,
                    access_token,
                    access_token_secret,
                    name,
                } => {
                    if let Ok(client) = XClient::new(
                        consumer_key.clone(),
                        consumer_secret.clone(),
                        access_token.clone(),
                        access_token_secret.clone(),
                        name.clone(),
                    ) {
                        sns_clients.push(Box::new(client));
                    }
                }
                _ => {}
            }
        }
    }

    let mut image_url = None;
    let mut media_paths = Vec::new();
    for file in &post.media_files {
        if file.starts_with("http://") || file.starts_with("https://") {
            if image_url.is_none() {
                image_url = Some(file.clone());
            }
        } else {
            media_paths.push(file.clone());
        }
    }
    let media_paths_opt = if media_paths.is_empty() {
        None
    } else {
        Some(media_paths)
    };

    let post_content = PostContent {
        text: post.content.clone(),
        image_url,
        media_paths: media_paths_opt,
        link_url: post.link_url.clone(),
        sensitive: post.sensitive,
    };

    let mut results = Vec::new();
    let mut failed_sns = Vec::new();

    for client in sns_clients {
        let target_name = client.account_name().to_string();
        match client.post(&post_content).await {
            Ok(result) => {
                if !result.success {
                    let err = result
                        .error_message
                        .clone()
                        .unwrap_or_else(|| "Unknown error".to_string());
                    failed_sns.push((target_name.clone(), err));
                }
                results.push(result);
            }
            Err(e) => {
                failed_sns.push((target_name.clone(), e.to_string()));
                results.push(PostResult {
                    success: false,
                    post_id: None,
                    error_message: Some(e.to_string()),
                });
            }
        }
    }

    let now_updated = chrono::Local::now();
    post.updated_at = now_updated;
    let all_success = failed_sns.is_empty();

    if all_success {
        post.status = "投稿済み".to_string();
        post.error_message = None;
    } else {
        post.status = "失敗".to_string();
        let errors: Vec<String> = failed_sns
            .into_iter()
            .map(|(sns, err)| format!("{}: {}", sns, err))
            .collect();
        post.error_message = Some(errors.join("; "));
    }

    let post_id = post.id.clone();
    state.store.update_post(&post_id, post).await.map_err(|e| {
        println!("Failed to update schedule status {}: {:?}", post_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ManualPostResponse {
        success: all_success,
        results,
    }))
}

#[derive(Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

pub async fn get_login_page() -> impl axum::response::IntoResponse {
    match std::fs::read_to_string("static/login.html") {
        Ok(html) => axum::response::Html(html).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Login page not found").into_response(),
    }
}

pub async fn login_submit(
    State(state): State<Arc<AppState>>,
    axum::Form(payload): axum::Form<LoginPayload>,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    let Some(ref auth) = state.config.web_auth else {
        println!("web_auth is not configured in config.yml");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    if payload.username != auth.username {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let mut verified = false;
    let mut needs_hash_migration = false;

    if auth.password.starts_with("$2b$")
        || auth.password.starts_with("$2a$")
        || auth.password.starts_with("$2y$")
    {
        if let Ok(ok) = bcrypt::verify(&payload.password, &auth.password) {
            verified = ok;
        }
    } else {
        if payload.password == auth.password {
            verified = true;
            needs_hash_migration = true;
        }
    }

    if !verified {
        return Err(StatusCode::UNAUTHORIZED);
    }

    if needs_hash_migration
        && let Ok(hashed) = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST)
    {
        println!(
            "Plaintext password detected in configuration. Automatically migrating to bcrypt hash."
        );
        let config_path = state.config_path.clone();
        let mut updated_config = state.config.clone();
        if let Some(ref mut c_auth) = updated_config.web_auth {
            c_auth.password = hashed;
        }
        match serde_yaml::to_string(&updated_config) {
            Ok(yaml) => {
                if let Err(e) = std::fs::write(&config_path, yaml) {
                    println!("Failed to write updated config: {:?}", e);
                }
            }
            Err(e) => println!("Failed to serialize config to YAML: {:?}", e),
        }
    }

    let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    timestamp.hash(&mut hasher);
    let random_val = hasher.finish();
    let session_id = format!("sess_{}_{:x}", timestamp, random_val);

    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id.clone(), payload.username);
    }

    let cookie = format!("session_id={}; Path=/; HttpOnly; SameSite=Lax", session_id);
    let response = axum::response::Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(axum::http::header::LOCATION, "/")
        .header(axum::http::header::SET_COOKIE, cookie)
        .body(axum::body::Body::empty())
        .unwrap();

    Ok(response)
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    req: axum::http::Request<axum::body::Body>,
) -> impl axum::response::IntoResponse {
    if let Some(cookie_header) = req.headers().get(axum::http::header::COOKIE)
        && let Ok(cookie_str) = cookie_header.to_str()
    {
        for cookie in cookie_str.split(';') {
            let parts: Vec<&str> = cookie.trim().split('=').collect();
            if parts.len() == 2 && parts[0] == "session_id" {
                let session_id = parts[1];
                let mut sessions = state.sessions.write().await;
                sessions.remove(session_id);
                break;
            }
        }
    }

    let cookie = "session_id=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT";
    axum::response::Response::builder()
        .status(axum::http::StatusCode::SEE_OTHER)
        .header(axum::http::header::LOCATION, "/login")
        .header(axum::http::header::SET_COOKIE, cookie)
        .body(axum::body::Body::empty())
        .unwrap()
}

use axum::extract::Query;
use axum::response::sse::{Event, Sse};
use serde_json::json;
use std::convert::Infallible;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

// SSE 接続が Drop (切断) された際に mcp_sessions からセッションを削除するラッパーストリーム
struct SessionCleanupStream<S> {
    inner: S,
    session_id: String,
    mcp_sessions: Arc<tokio::sync::RwLock<HashMap<String, tokio::sync::mpsc::Sender<Event>>>>,
}

impl<S> Drop for SessionCleanupStream<S> {
    fn drop(&mut self) {
        let mcp_sessions = self.mcp_sessions.clone();
        let session_id = self.session_id.clone();
        tokio::spawn(async move {
            let mut guard = mcp_sessions.write().await;
            guard.remove(&session_id);
            println!("MCP SSE Session disconnected & cleaned up: {}", session_id);
        });
    }
}

use std::pin::Pin;
use std::task::{Context, Poll};

impl<S: tokio_stream::Stream + Unpin> tokio_stream::Stream for SessionCleanupStream<S> {
    type Item = S::Item;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

// GET /api/mcp/sse
pub async fn mcp_sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let session_id = format!("mcp-{}", chrono::Utc::now().timestamp_micros());
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    // セッションを登録
    {
        let mut mcp_sessions = state.mcp_sessions.write().await;
        mcp_sessions.insert(session_id.clone(), tx.clone());
        println!("MCP SSE Session connected: {}", session_id);
    }

    // 最初の endpoint イベントを送信して、クライアントへメッセージ送信先を指定する
    let endpoint_url = format!("/api/mcp/message?session_id={}", session_id);
    let init_event = Event::default().event("endpoint").data(endpoint_url);

    let _ = tx.send(init_event).await;

    // ストリームの作成
    let rx_stream = ReceiverStream::new(rx).map(Ok);

    // 切断検知時にセッション削除するストリームに変換
    let clean_stream = SessionCleanupStream {
        inner: rx_stream,
        session_id,
        mcp_sessions: state.mcp_sessions.clone(),
    };

    Sse::new(clean_stream).keep_alive(axum::response::sse::KeepAlive::default())
}

#[derive(Deserialize)]
pub struct McpQuery {
    pub session_id: String,
}

// POST /api/mcp/message
pub async fn mcp_message_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<McpQuery>,
    Json(rpc_req): Json<serde_json::Value>,
) -> impl axum::response::IntoResponse {
    let state_clone = state.clone();
    let session_id = query.session_id.clone();

    tokio::spawn(async move {
        if let Err(e) = handle_mcp_request(state_clone, &session_id, rpc_req).await {
            println!(
                "Error handling MCP request for session {}: {:?}",
                session_id, e
            );
        }
    });

    StatusCode::ACCEPTED
}

async fn handle_mcp_request(
    state: Arc<AppState>,
    session_id: &str,
    req: serde_json::Value,
) -> anyhow::Result<()> {
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = req.get("id").cloned();
    let has_id = id.is_some();

    let response = if method == "initialize" {
        json!({
            "jsonrpc": "2.0",
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "blog-autopost-rs",
                    "version": "0.1.0"
                }
            },
            "id": id
        })
    } else if method == "initialized" {
        return Ok(());
    } else if method == "tools/list" {
        json!({
            "jsonrpc": "2.0",
            "result": {
                "tools": [
                    {
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
                    },
                    {
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
                    },
                    {
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
                    },
                    {
                        "name": "delete_schedule",
                        "description": "指定したIDの予約投稿を削除します。",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string", "description": "削除対象の予約投稿ID" }
                            },
                            "required": ["id"]
                        }
                    },
                    {
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
                    }
                ]
            },
            "id": id
        })
    } else if method == "tools/call" {
        let params = req.get("params");
        let name = params
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("");
        let arguments = params
            .and_then(|p| p.get("arguments"))
            .cloned()
            .unwrap_or(json!({}));

        let result = handle_tool_call(state.clone(), name, arguments).await;

        match result {
            Ok(res_val) => {
                json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": res_val
                            }
                        ]
                    },
                    "id": id
                })
            }
            Err(e) => {
                json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": format!("Tool execution error: {:?}", e)
                    },
                    "id": id
                })
            }
        }
    } else {
        json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32601,
                "message": format!("Method not found: {}", method)
            },
            "id": id
        })
    };

    if has_id {
        let mcp_sessions = state.mcp_sessions.read().await;
        if let Some(tx) = mcp_sessions.get(session_id) {
            let event = Event::default()
                .event("message")
                .data(serde_json::to_string(&response)?);
            let _ = tx.send(event).await;
        }
    }

    Ok(())
}

async fn handle_tool_call(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sns() -> Vec<SnsConfig> {
        vec![
            SnsConfig::X {
                name: "x".to_string(),
                consumer_key: "a".to_string(),
                consumer_secret: "b".to_string(),
                access_token: "c".to_string(),
                access_token_secret: "d".to_string(),
            },
            SnsConfig::Bluesky {
                name: "bluesky".to_string(),
                identifier: "e".to_string(),
                password: "f".to_string(),
            },
            SnsConfig::Mastodon {
                name: "mastodon-social".to_string(),
                instance_url: "https://mastodon.social".to_string(),
                access_token: "g".to_string(),
            },
            SnsConfig::Misskey {
                name: "misskey-io".to_string(),
                instance_url: "https://misskey.io".to_string(),
                access_token: "h".to_string(),
                is_sensitive: None,
            },
            SnsConfig::Unknown,
        ]
    }

    #[test]
    fn sns_account_name_は既知種別の名前を返す() {
        let sns = sample_sns();
        assert_eq!(sns_account_name(&sns[0]), Some("x"));
        assert_eq!(sns_account_name(&sns[2]), Some("mastodon-social"));
        assert_eq!(sns_account_name(&SnsConfig::Unknown), None);
    }

    #[test]
    fn sns_display_label_は種別と名前を組み合わせる() {
        let sns = sample_sns();
        assert_eq!(sns_display_label(&sns[0]).as_deref(), Some("X (x)"));
        assert_eq!(
            sns_display_label(&sns[3]).as_deref(),
            Some("Misskey (misskey-io)")
        );
        assert_eq!(sns_display_label(&SnsConfig::Unknown), None);
    }

    #[test]
    fn build_sns_accounts_は未知種別を除いた全件を返す() {
        let accounts = build_sns_accounts(&sample_sns());
        assert_eq!(accounts.len(), 4);
        let names: Vec<&str> = accounts.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["x", "bluesky", "mastodon-social", "misskey-io"]);
        assert_eq!(accounts[0].sns_type, "x");
        assert_eq!(accounts[0].label, "X (x)");
    }

    #[test]
    fn build_sns_accounts_は括弧入りの名前をそのまま保持する() {
        let sns = vec![SnsConfig::Mastodon {
            name: "my(test)".to_string(),
            instance_url: "https://example.com".to_string(),
            access_token: "t".to_string(),
        }];
        let accounts = build_sns_accounts(&sns);
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].name, "my(test)");
        assert_eq!(accounts[0].label, "Mastodon (my(test))");
    }

    #[test]
    fn resolve_sns_name_は表示ラベルでもアカウント名でも解決する() {
        let sns = sample_sns();
        assert_eq!(resolve_sns_name(&sns, "X (x)").as_deref(), Some("x"));
        assert_eq!(resolve_sns_name(&sns, "x").as_deref(), Some("x"));
        assert_eq!(
            resolve_sns_name(&sns, "misskey-io").as_deref(),
            Some("misskey-io")
        );
    }

    #[test]
    fn resolve_sns_name_は括弧入りの名前も解決する() {
        let sns = vec![SnsConfig::Mastodon {
            name: "my(test)".to_string(),
            instance_url: "https://example.com".to_string(),
            access_token: "t".to_string(),
        }];
        assert_eq!(
            resolve_sns_name(&sns, "my(test)").as_deref(),
            Some("my(test)")
        );
        assert_eq!(
            resolve_sns_name(&sns, "Mastodon (my(test))").as_deref(),
            Some("my(test)")
        );
    }

    #[test]
    fn resolve_sns_name_は未知の指定を解決しない() {
        let sns = sample_sns();
        assert_eq!(resolve_sns_name(&sns, "nope"), None);
        assert_eq!(resolve_sns_name(&sns, "Unknown"), None);
    }
}
