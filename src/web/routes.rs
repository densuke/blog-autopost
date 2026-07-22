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

#[derive(Serialize)]
pub struct ConfigResponse {
    pub blog_name: String,
    pub active_sns: Vec<String>,
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> Json<ConfigResponse> {
    let blog_name = state
        .config
        .blog
        .as_ref()
        .and_then(|b| b.first())
        .map(|b| b.name.clone())
        .unwrap_or_else(|| "Unknown Blog".to_string());

    let active_sns = state
        .config
        .sns
        .iter()
        .map(|s| match s {
            SnsConfig::Mastodon { name, .. } => format!("Mastodon ({})", name),
            SnsConfig::Misskey { name, .. } => format!("Misskey ({})", name),
            SnsConfig::Bluesky { name, .. } => format!("Bluesky ({})", name),
            SnsConfig::X { name, .. } => format!("X ({})", name),
            _ => "Unknown".to_string(),
        })
        .collect();

    Json(ConfigResponse {
        blog_name,
        active_sns,
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
        let target_name = match sns_conf {
            SnsConfig::Mastodon { name, .. } => format!("Mastodon ({})", name),
            SnsConfig::Misskey { name, .. } => format!("Misskey ({})", name),
            SnsConfig::Bluesky { name, .. } => format!("Bluesky ({})", name),
            SnsConfig::X { name, .. } => format!("X ({})", name),
            _ => continue,
        };

        if let Some(ref selected) = payload.targets
            && !selected.contains(&target_name)
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
            let sns_name = state.config.sns.iter().find_map(|s| {
                let name = match s {
                    SnsConfig::Mastodon { name, .. } => name,
                    SnsConfig::Misskey { name, .. } => name,
                    SnsConfig::Bluesky { name, .. } => name,
                    SnsConfig::X { name, .. } => name,
                    SnsConfig::Threads { name, .. } => name,
                    SnsConfig::Tumblr { name, .. } => name,
                    _ => return None,
                };
                let formatted = match s {
                    SnsConfig::Mastodon { .. } => format!("Mastodon ({})", name),
                    SnsConfig::Misskey { .. } => format!("Misskey ({})", name),
                    SnsConfig::Bluesky { .. } => format!("Bluesky ({})", name),
                    SnsConfig::X { .. } => format!("X ({})", name),
                    SnsConfig::Threads { .. } => format!("Threads ({})", name),
                    SnsConfig::Tumblr { .. } => format!("Tumblr ({})", name),
                    _ => return None,
                };
                if formatted == *target {
                    Some(name.clone())
                } else {
                    None
                }
            });

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
        let name = match sns_conf {
            SnsConfig::Mastodon { name, .. } => name,
            SnsConfig::Misskey { name, .. } => name,
            SnsConfig::Bluesky { name, .. } => name,
            SnsConfig::X { name, .. } => name,
            SnsConfig::Threads { name, .. } => name,
            SnsConfig::Tumblr { name, .. } => name,
            _ => continue,
        };

        let slot = finder
            .find_next_available_slot(name, None, 7)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        slots.insert(name.clone(), slot.map(|dt| dt.to_rfc3339()));
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
    use crate::scheduled::ScheduledPost;
    use crate::web::tests::{
        TEST_PASSWORD, TEST_USERNAME, TestApp, setup_test_app, setup_test_app_with_config,
    };
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use tower::ServiceExt;

    const SECRET: &str = "test-secret-token";

    fn app_with_auth() -> TestApp {
        setup_test_app(Some(SECRET.to_string()))
    }

    /// APIキー付きのGETリクエストを作る。
    fn api_get(uri: &str) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .header("X-Api-Key", SECRET)
            .body(Body::empty())
            .unwrap()
    }

    /// 応答ボディをJSONとして読み出す。
    async fn json_body(response: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("ボディの読み出しに失敗");
        serde_json::from_slice(&bytes).expect("JSONとして解釈できない")
    }

    /// 予約を1件用意し、そのIDを返す。
    async fn seed(app: &TestApp, content: &str) -> String {
        let post = ScheduledPost::new(
            content.to_string(),
            chrono::Local::now() + chrono::Duration::hours(1),
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

    // --- GET /api/config ---

    /// SNS未設定・blog未設定の場合は既定値が返る。
    #[tokio::test]
    async fn test_get_config_defaults() {
        let app = app_with_auth();

        let response = app
            .router
            .clone()
            .oneshot(api_get("/api/config"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["blog_name"], "Unknown Blog");
        assert_eq!(body["active_sns"].as_array().unwrap().len(), 0);
    }

    /// 設定済みのブログ名とSNS一覧が返る。
    #[tokio::test]
    async fn test_get_config_with_blog_and_sns() {
        use crate::config::{BlogConfig, SnsConfig};
        use std::collections::HashMap;

        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.blog = Some(vec![BlogConfig {
                name: "テストブログ".to_string(),
                feed_url: "https://example.com/feed".to_string(),
                extra: HashMap::new(),
            }]);
            config.sns = vec![
                SnsConfig::Mastodon {
                    name: "mstdn-main".to_string(),
                    instance_url: "https://mstdn.example.com".to_string(),
                    access_token: "t".to_string(),
                },
                SnsConfig::Bluesky {
                    name: "bsky-main".to_string(),
                    identifier: "id".to_string(),
                    password: "pw".to_string(),
                },
                SnsConfig::Unknown,
            ];
        });

        let response = app
            .router
            .clone()
            .oneshot(api_get("/api/config"))
            .await
            .unwrap();

        let body = json_body(response).await;
        assert_eq!(body["blog_name"], "テストブログ");
        let sns = body["active_sns"].as_array().unwrap();
        assert_eq!(sns[0], "Mastodon (mstdn-main)");
        assert_eq!(sns[1], "Bluesky (bsky-main)");
        assert_eq!(sns[2], "Unknown");
    }

    // --- GET /api/schedules ---

    #[tokio::test]
    async fn test_get_schedules_empty() {
        let app = app_with_auth();

        let response = app
            .router
            .clone()
            .oneshot(api_get("/api/schedules"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_get_schedules_returns_created_posts() {
        let app = app_with_auth();
        seed(&app, "予約1").await;
        seed(&app, "予約2").await;

        let response = app
            .router
            .clone()
            .oneshot(api_get("/api/schedules"))
            .await
            .unwrap();

        let body = json_body(response).await;
        let posts = body.as_array().unwrap();
        assert_eq!(posts.len(), 2);
        assert_eq!(posts[0]["content"], "予約1");
    }

    // --- PUT /api/schedules/{id} ---

    #[tokio::test]
    async fn test_update_schedule_success() {
        let app = app_with_auth();
        let id = seed(&app, "変更前").await;

        let payload = serde_json::json!({
            "content": "変更後",
            "scheduled_at": "2026-09-01T09:00:00+09:00",
            "target_sns": ["mastodon", "bluesky"],
            "status": "投稿済み",
            "media_files": ["a.png"],
            "link_url": "https://example.com/a"
        });

        let request = Request::builder()
            .method("PUT")
            .uri(format!("/api/schedules/{}", id))
            .header("X-Api-Key", SECRET)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["content"], "変更後");
        assert_eq!(body["status"], "投稿済み");
        assert_eq!(body["target_sns"][0], "mastodon");
        assert_eq!(body["media_files"][0], "a.png");
        assert_eq!(body["link_url"], "https://example.com/a");
    }

    /// 存在しないIDの更新は404を返す。
    #[tokio::test]
    async fn test_update_schedule_not_found() {
        let app = app_with_auth();

        let payload = serde_json::json!({
            "content": "x",
            "scheduled_at": "2026-09-01T09:00:00+09:00",
            "target_sns": ["mastodon"],
            "status": "予約済み"
        });

        let request = Request::builder()
            .method("PUT")
            .uri("/api/schedules/post-does-not-exist")
            .header("X-Api-Key", SECRET)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    /// 日時の形式が不正な場合は400を返す。
    #[tokio::test]
    async fn test_update_schedule_invalid_datetime() {
        let app = app_with_auth();
        let id = seed(&app, "変更前").await;

        let payload = serde_json::json!({
            "content": "x",
            "scheduled_at": "めちゃくちゃな日時",
            "target_sns": ["mastodon"],
            "status": "予約済み"
        });

        let request = Request::builder()
            .method("PUT")
            .uri(format!("/api/schedules/{}", id))
            .header("X-Api-Key", SECRET)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // --- DELETE /api/schedules/{id} ---

    #[tokio::test]
    async fn test_delete_schedule_success() {
        let app = app_with_auth();
        let id = seed(&app, "消す予約").await;

        let request = Request::builder()
            .method("DELETE")
            .uri(format!("/api/schedules/{}", id))
            .header("X-Api-Key", SECRET)
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(app.state.store.get_all_posts().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_delete_schedule_not_found() {
        let app = app_with_auth();

        let request = Request::builder()
            .method("DELETE")
            .uri("/api/schedules/post-does-not-exist")
            .header("X-Api-Key", SECRET)
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // --- GET /api/next-slots ---

    #[tokio::test]
    async fn test_get_next_slots() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.default_allowed_timings = Some(vec![(
                "*".to_string(),
                vec!["09:00".to_string(), "18:00".to_string()],
            )]);
            config.sns = vec![crate::config::SnsConfig::Mastodon {
                name: "mstdn-main".to_string(),
                instance_url: "https://mstdn.example.com".to_string(),
                access_token: "t".to_string(),
            }];
        });

        let response = app
            .router
            .clone()
            .oneshot(api_get("/api/next-slots"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;

        // 応答は {"slots": {"<SNS名>": "<RFC3339の日時>"}} の形
        let slots = body["slots"].as_object().expect("slotsはオブジェクト");
        let slot = slots
            .get("mstdn-main")
            .expect("設定したSNSの枠が含まれるはず");
        let slot_str = slot.as_str().expect("枠が見つかるはずなので日時文字列");
        assert!(
            chrono::DateTime::parse_from_rfc3339(slot_str).is_ok(),
            "RFC3339として解釈できること: {}",
            slot_str
        );
        // 設定した 09:00 / 18:00 のいずれかの枠が返る
        assert!(
            slot_str.contains("T09:00:00") || slot_str.contains("T18:00:00"),
            "設定したタイミングの枠であること: {}",
            slot_str
        );
    }

    /// SNSが未設定なら枠も空になる。
    #[tokio::test]
    async fn test_get_next_slots_without_sns() {
        let app = app_with_auth();

        let response = app
            .router
            .clone()
            .oneshot(api_get("/api/next-slots"))
            .await
            .unwrap();

        let body = json_body(response).await;
        assert!(body["slots"].as_object().unwrap().is_empty());
    }

    // --- 認証 ---

    /// APIキーが無いと401になる。
    #[tokio::test]
    async fn test_api_requires_auth() {
        let app = app_with_auth();

        for uri in ["/api/config", "/api/schedules", "/api/next-slots"] {
            let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
            let response = app.router.clone().oneshot(request).await.unwrap();

            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{} は認証が必要なはず",
                uri
            );
        }
    }

    /// 誤ったAPIキーでも401になる。
    #[tokio::test]
    async fn test_api_rejects_wrong_key() {
        let app = app_with_auth();

        let request = Request::builder()
            .uri("/api/config")
            .header("X-Api-Key", "wrong-key")
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // --- POST /login ---

    /// 正しい資格情報でログインするとセッションが作られ、Cookieが返る。
    #[tokio::test]
    async fn test_login_success_sets_session_cookie() {
        let app = app_with_auth();

        let request = Request::builder()
            .method("POST")
            .uri("/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "username={}&password={}",
                TEST_USERNAME, TEST_PASSWORD
            )))
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers().get(header::LOCATION).unwrap(), "/");

        let cookie = response
            .headers()
            .get(header::SET_COOKIE)
            .expect("Set-Cookieが必要")
            .to_str()
            .unwrap();
        assert!(
            cookie.starts_with("session_id=sess_"),
            "実際の値: {}",
            cookie
        );
        assert!(cookie.contains("HttpOnly"), "HttpOnly属性が必要");

        assert_eq!(app.state.sessions.read().await.len(), 1);
    }

    /// パスワードが違うと401になり、セッションは作られない。
    #[tokio::test]
    async fn test_login_with_wrong_password() {
        let app = app_with_auth();

        let request = Request::builder()
            .method("POST")
            .uri("/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "username={}&password=wrong",
                TEST_USERNAME
            )))
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(app.state.sessions.read().await.is_empty());
    }

    /// ユーザー名が違うと401になる。
    #[tokio::test]
    async fn test_login_with_wrong_username() {
        let app = app_with_auth();

        let request = Request::builder()
            .method("POST")
            .uri("/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "username=someone&password={}",
                TEST_PASSWORD
            )))
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    /// web_auth 未設定の場合は500になる。
    #[tokio::test]
    async fn test_login_without_web_auth_config() {
        let app = setup_test_app_with_config(None, |config| {
            config.web_auth = None;
        });

        let request = Request::builder()
            .method("POST")
            .uri("/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("username=admin&password=password"))
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    /// 平文パスワードの設定でログインすると、bcryptハッシュへ自動移行される。
    #[tokio::test]
    async fn test_login_migrates_plaintext_password() {
        let app = setup_test_app_with_config(None, |config| {
            if let Some(ref mut auth) = config.web_auth {
                auth.password = "plaintext".to_string();
            }
        });

        let request = Request::builder()
            .method("POST")
            .uri("/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("username=admin&password=plaintext"))
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);

        // 設定ファイルがbcryptハッシュへ書き換えられている
        let written = std::fs::read_to_string(&app.state.config_path)
            .expect("設定ファイルが書き出されているはず");
        assert!(
            written.contains("$2b$") || written.contains("$2y$") || written.contains("$2a$"),
            "bcryptハッシュへ移行されるはず: {}",
            written
        );
        assert!(!written.contains("plaintext"), "平文が残ってはいけない");
    }

    // --- GET /logout ---

    /// ログアウトするとセッションが破棄される。
    #[tokio::test]
    async fn test_logout_removes_session() {
        let app = app_with_auth();
        {
            let mut sessions = app.state.sessions.write().await;
            sessions.insert("my-session".to_string(), "admin".to_string());
        }

        let request = Request::builder()
            .uri("/logout")
            .header(header::COOKIE, "session_id=my-session")
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert!(app.state.sessions.read().await.is_empty());
    }

    // --- GET /login (ページ) ---

    // --- POST /api/post (即時投稿) ---

    /// Mastodonの投稿先をモックサーバへ向けたテスト環境を作る。
    fn app_with_mock_mastodon(server: &wiremock::MockServer) -> TestApp {
        let uri = server.uri();
        setup_test_app_with_config(Some(SECRET.to_string()), move |config| {
            config.sns = vec![crate::config::SnsConfig::Mastodon {
                name: "mstdn-main".to_string(),
                instance_url: uri,
                access_token: "t".to_string(),
            }];
        })
    }

    fn post_json(uri: &str, payload: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("X-Api-Key", SECRET)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap()
    }

    /// 即時投稿が成功すると success:true と結果が返る。
    #[tokio::test]
    async fn test_manual_post_immediate_success() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "url": "https://mstdn.example.com/@u/1" })),
            )
            .mount(&server)
            .await;

        let app = app_with_mock_mastodon(&server);

        let response = app
            .router
            .clone()
            .oneshot(post_json(
                "/api/post",
                serde_json::json!({ "text": "即時投稿のテスト" }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["success"], true);
        assert_eq!(body["results"][0]["success"], true);
        assert_eq!(
            body["results"][0]["post_id"],
            "https://mstdn.example.com/@u/1"
        );
    }

    /// 投稿先がエラーを返した場合は success:false になる。
    #[tokio::test]
    async fn test_manual_post_immediate_failure() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            .mount(&server)
            .await;

        let app = app_with_mock_mastodon(&server);

        let response = app
            .router
            .clone()
            .oneshot(post_json(
                "/api/post",
                serde_json::json!({ "text": "失敗する投稿" }),
            ))
            .await
            .unwrap();

        let body = json_body(response).await;
        assert_eq!(body["success"], false);
        assert_eq!(body["results"][0]["success"], false);
    }

    /// targets で投稿先を絞り込める。該当しなければ投稿されない。
    #[tokio::test]
    async fn test_manual_post_filters_by_targets() {
        use wiremock::MockServer;

        let server = MockServer::start().await;
        let app = app_with_mock_mastodon(&server);

        let response = app
            .router
            .clone()
            .oneshot(post_json(
                "/api/post",
                serde_json::json!({
                    "text": "対象外",
                    "targets": ["Misskey (misskey-main)"]
                }),
            ))
            .await
            .unwrap();

        let body = json_body(response).await;
        assert_eq!(
            body["results"].as_array().unwrap().len(),
            0,
            "対象が一致しないので投稿されないはず"
        );
    }

    // --- POST /api/post (予約投稿) ---

    /// schedule_type=custom で日時を指定すると予約が作られる。
    #[tokio::test]
    async fn test_manual_post_schedules_custom_time() {
        use wiremock::MockServer;

        let server = MockServer::start().await;
        let app = app_with_mock_mastodon(&server);

        let response = app
            .router
            .clone()
            .oneshot(post_json(
                "/api/post",
                serde_json::json!({
                    "text": "予約投稿のテスト",
                    "targets": ["Mastodon (mstdn-main)"],
                    "schedule_type": "custom",
                    "scheduled_at": "2026-09-01T09:00:00+09:00"
                }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["success"], true);

        let posts = app.state.store.get_all_posts().await.unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].content, "予約投稿のテスト");
        assert_eq!(posts[0].target_sns, vec!["mstdn-main"]);
    }

    /// schedule_type=custom で scheduled_at が無い場合は失敗する。
    #[tokio::test]
    async fn test_manual_post_schedule_without_time() {
        use wiremock::MockServer;

        let server = MockServer::start().await;
        let app = app_with_mock_mastodon(&server);

        let response = app
            .router
            .clone()
            .oneshot(post_json(
                "/api/post",
                serde_json::json!({
                    "text": "時刻なし",
                    "targets": ["Mastodon (mstdn-main)"],
                    "schedule_type": "custom"
                }),
            ))
            .await
            .unwrap();

        let body = json_body(response).await;
        assert_eq!(body["success"], false);
        assert!(app.state.store.get_all_posts().await.unwrap().is_empty());
    }

    /// 不正な日時形式では予約されない。
    #[tokio::test]
    async fn test_manual_post_schedule_invalid_datetime() {
        use wiremock::MockServer;

        let server = MockServer::start().await;
        let app = app_with_mock_mastodon(&server);

        let response = app
            .router
            .clone()
            .oneshot(post_json(
                "/api/post",
                serde_json::json!({
                    "text": "不正な日時",
                    "targets": ["Mastodon (mstdn-main)"],
                    "schedule_type": "custom",
                    "scheduled_at": "めちゃくちゃな日時"
                }),
            ))
            .await
            .unwrap();

        let body = json_body(response).await;
        assert_eq!(body["success"], false);
        assert!(app.state.store.get_all_posts().await.unwrap().is_empty());
    }

    /// 予約時に targets が空だと失敗する。
    #[tokio::test]
    async fn test_manual_post_schedule_without_targets() {
        use wiremock::MockServer;

        let server = MockServer::start().await;
        let app = app_with_mock_mastodon(&server);

        let response = app
            .router
            .clone()
            .oneshot(post_json(
                "/api/post",
                serde_json::json!({
                    "text": "対象なし",
                    "schedule_type": "custom",
                    "scheduled_at": "2026-09-01T09:00:00+09:00"
                }),
            ))
            .await
            .unwrap();

        let body = json_body(response).await;
        assert_eq!(body["success"], false);
    }

    /// schedule_type=next では次の空き枠が自動で選ばれる。
    #[tokio::test]
    async fn test_manual_post_schedules_next_slot() {
        use wiremock::MockServer;

        let server = MockServer::start().await;
        let uri = server.uri();
        let app = setup_test_app_with_config(Some(SECRET.to_string()), move |config| {
            config.default_allowed_timings = Some(vec![(
                "*".to_string(),
                vec!["09:00".to_string(), "18:00".to_string()],
            )]);
            config.sns = vec![crate::config::SnsConfig::Mastodon {
                name: "mstdn-main".to_string(),
                instance_url: uri,
                access_token: "t".to_string(),
            }];
        });

        let response = app
            .router
            .clone()
            .oneshot(post_json(
                "/api/post",
                serde_json::json!({
                    "text": "次の枠へ予約",
                    "targets": ["Mastodon (mstdn-main)"],
                    "schedule_type": "next"
                }),
            ))
            .await
            .unwrap();

        let body = json_body(response).await;
        assert_eq!(body["success"], true, "実際の応答: {}", body);

        let posts = app.state.store.get_all_posts().await.unwrap();
        assert_eq!(posts.len(), 1);
        let hhmm = posts[0].scheduled_at.format("%H:%M").to_string();
        assert!(hhmm == "09:00" || hhmm == "18:00", "実際の時刻: {}", hhmm);
    }

    // --- POST /api/schedules/{id}/post-now ---

    /// 予約を即時投稿すると、投稿先へ送信され結果が返る。
    #[tokio::test]
    async fn test_post_now_schedule_success() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "url": "https://mstdn.example.com/@u/9" })),
            )
            .mount(&server)
            .await;

        let app = app_with_mock_mastodon(&server);

        // 対象SNSを設定名に合わせて予約を作る
        let post = ScheduledPost::new(
            "即時送信する予約".to_string(),
            chrono::Local::now() + chrono::Duration::hours(1),
            vec![],
            vec!["mstdn-main".to_string()],
        );
        let id = app.state.store.create_post(post).await.unwrap().id;

        let request = Request::builder()
            .method("POST")
            .uri(format!("/api/schedules/{}/post-now", id))
            .header("X-Api-Key", SECRET)
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["success"], true, "実際の応答: {}", body);
    }

    /// 存在しない予約の即時投稿は404を返す。
    #[tokio::test]
    async fn test_post_now_schedule_not_found() {
        let app = app_with_auth();

        let request = Request::builder()
            .method("POST")
            .uri("/api/schedules/post-does-not-exist/post-now")
            .header("X-Api-Key", SECRET)
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // --- MCP ---

    /// MCPのメッセージ受付は 202 Accepted を返す(処理は非同期)。
    #[tokio::test]
    async fn test_mcp_message_returns_accepted() {
        let app = app_with_auth();

        let request = Request::builder()
            .method("POST")
            .uri("/api/mcp/message?session_id=test-session")
            .header("X-Api-Key", SECRET)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "initialize",
                    "id": 1
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    /// tools/list も同様に受け付けられる。
    #[tokio::test]
    async fn test_mcp_message_tools_list() {
        let app = app_with_auth();

        let request = Request::builder()
            .method("POST")
            .uri("/api/mcp/message?session_id=test-session")
            .header("X-Api-Key", SECRET)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "tools/list",
                    "id": 2
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    // --- POST /api/upload ---

    /// 許可されていない形式のファイルは拒否される。
    #[tokio::test]
    async fn test_upload_rejects_disallowed_mime() {
        let app = app_with_auth();

        let boundary = "X-BOUNDARY";
        let body = format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--{b}--\r\n",
            b = boundary
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/upload")
            .header("X-Api-Key", SECRET)
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body))
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = json_body(response).await;
        assert_eq!(json["success"], false);
        let err = json["error"].as_str().expect("エラーメッセージが必要");
        assert!(
            err.contains("許可されていないファイル形式"),
            "実際の値: {}",
            err
        );
    }

    /// static/login.html が存在すればHTMLを返す。
    /// テスト実行時のカレントディレクトリによって結果が変わるため、
    /// 200 か 404 のいずれかであることのみを確認する。
    #[tokio::test]
    async fn test_get_login_page_responds() {
        let app = app_with_auth();

        let request = Request::builder()
            .uri("/login")
            .body(Body::empty())
            .unwrap();
        let response = app.router.clone().oneshot(request).await.unwrap();

        assert!(
            response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND,
            "実際のステータス: {}",
            response.status()
        );
    }
}
