use std::sync::Arc;
use std::collections::HashMap;
use axum::{
    extract::{State, Path, Multipart},
    Json,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::sns::models::{PostContent, PostResult};
use crate::sns::traits::SnsClient;
use crate::config::SnsConfig;
use crate::sns::{mastodon::MastodonClient, misskey::MisskeyClient, bluesky::BlueskyClient, x::XClient};
use super::AppState;

#[derive(Serialize)]
pub struct ConfigResponse {
    pub blog_name: String,
    pub active_sns: Vec<String>,
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> Json<ConfigResponse> {
    let blog_name = state.config.blog.as_ref()
        .and_then(|b| b.first())
        .map(|b| b.name.clone())
        .unwrap_or_else(|| "Unknown Blog".to_string());

    let active_sns = state.config.sns.iter().map(|s| match s {
        SnsConfig::Mastodon { name, .. } => format!("Mastodon ({})", name),
        SnsConfig::Misskey { name, .. } => format!("Misskey ({})", name),
        SnsConfig::Bluesky { name, .. } => format!("Bluesky ({})", name),
        SnsConfig::X { name, .. } => format!("X ({})", name),
        _ => "Unknown".to_string(),
    }).collect();

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

        if let Some(ref selected) = payload.targets {
            if !selected.contains(&target_name) {
                continue;
            }
        }

        match sns_conf {
            SnsConfig::Mastodon { instance_url, access_token, name, .. } => {
                if let Ok(client) = MastodonClient::new(instance_url.clone(), access_token.clone(), name.clone()) {
                    sns_clients.push(Box::new(client));
                }
            }
            SnsConfig::Misskey { instance_url, access_token, name, .. } => {
                if let Ok(client) = MisskeyClient::new(instance_url.clone(), access_token.clone(), name.clone()) {
                    sns_clients.push(Box::new(client));
                }
            }
            SnsConfig::Bluesky { identifier, password, name, .. } => {
                if let Ok(client) = BlueskyClient::new(identifier.clone(), password.clone(), name.clone()) {
                    sns_clients.push(Box::new(client));
                }
            }
            SnsConfig::X { consumer_key, consumer_secret, access_token, access_token_secret, name } => {
                if let Ok(client) = XClient::new(consumer_key.clone(), consumer_secret.clone(), access_token.clone(), access_token_secret.clone(), name.clone()) {
                    sns_clients.push(Box::new(client));
                }
            }
            _ => {}
        }
    }

    let schedule_type = payload.schedule_type.clone().unwrap_or_else(|| "now".to_string());

    if schedule_type == "now" {
        let post_content = PostContent {
            text: payload.text,
            image_url: payload.image_url,
            media_paths: payload.media_paths,
            link_url: payload.link_url,
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
                            error_message: Some(format!("Failed to calculate slot for {}: {}", target, e)),
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
                        error_message: Some("Missing scheduled_at time for custom schedule".to_string()),
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
            if media_files.is_empty() {
                if let Some(img_url) = &payload.image_url {
                    media_files.push(img_url.clone());
                }
            }
            let mut post = ScheduledPost::new(
                payload.text.clone(),
                scheduled_time,
                media_files,
                vec![sns_name.clone()],
            );
            post.link_url = payload.link_url.clone();

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

        let slot = finder.find_next_available_slot(name, None, 7).await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        slots.insert(name.clone(), slot.map(|dt| dt.to_rfc3339()));
    }

    Ok(Json(NextSlotResponse { slots }))
}

// GET /api/schedules
pub async fn get_schedules(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::scheduled::ScheduledPost>>, StatusCode> {
    state.store.get_all_posts().await
        .map(Json)
        .map_err(|e| {
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

    let existing = state.store.get_post_by_id(&id).await
        .map_err(|e| {
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

    let updated = state.store.update_post(&id, post).await
        .map_err(|e| {
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
    let success = state.store.delete_post(&id).await
        .map_err(|e| {
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

pub async fn upload_media(
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, StatusCode> {
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
        let content_type = field.content_type().unwrap_or("application/octet-stream").to_string();

        let allowed_types = [
            "image/jpeg",
            "image/png",
            "image/gif",
            "image/webp",
            "video/mp4",
            "video/quicktime",
        ];
        
        let mime_base = content_type.split(';').next().unwrap_or("").trim().to_lowercase();
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
                error: Some(format!("ファイルサイズが上限（10MB）を超えています: {} bytes", bytes.len())),
            }));
        }

        let sanitized_name: String = file_name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
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
    let existing = state.store.get_post_by_id(&id).await
        .map_err(|e| {
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
                SnsConfig::Mastodon { instance_url, access_token, name, .. } => {
                    if let Ok(client) = MastodonClient::new(instance_url.clone(), access_token.clone(), name.clone()) {
                        sns_clients.push(Box::new(client));
                    }
                }
                SnsConfig::Misskey { instance_url, access_token, name, .. } => {
                    if let Ok(client) = MisskeyClient::new(instance_url.clone(), access_token.clone(), name.clone()) {
                        sns_clients.push(Box::new(client));
                    }
                }
                SnsConfig::Bluesky { identifier, password, name, .. } => {
                    if let Ok(client) = BlueskyClient::new(identifier.clone(), password.clone(), name.clone()) {
                        sns_clients.push(Box::new(client));
                    }
                }
                SnsConfig::X { consumer_key, consumer_secret, access_token, access_token_secret, name } => {
                    if let Ok(client) = XClient::new(consumer_key.clone(), consumer_secret.clone(), access_token.clone(), access_token_secret.clone(), name.clone()) {
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
    let media_paths_opt = if media_paths.is_empty() { None } else { Some(media_paths) };

    let post_content = PostContent {
        text: post.content.clone(),
        image_url,
        media_paths: media_paths_opt,
        link_url: post.link_url.clone(),
    };

    let mut results = Vec::new();
    let mut failed_sns = Vec::new();

    for client in sns_clients {
        let target_name = client.account_name().to_string();
        match client.post(&post_content).await {
            Ok(result) => {
                if !result.success {
                    let err = result.error_message.clone().unwrap_or_else(|| "Unknown error".to_string());
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
        let errors: Vec<String> = failed_sns.into_iter().map(|(sns, err)| format!("{}: {}", sns, err)).collect();
        post.error_message = Some(errors.join("; "));
    }

    let post_id = post.id.clone();
    state.store.update_post(&post_id, post).await
        .map_err(|e| {
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

    if auth.password.starts_with("$2b$") || auth.password.starts_with("$2a$") || auth.password.starts_with("$2y$") {
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

    if needs_hash_migration {
        if let Ok(hashed) = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST) {
            println!("Plaintext password detected in configuration. Automatically migrating to bcrypt hash.");
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
    if let Some(cookie_header) = req.headers().get(axum::http::header::COOKIE) {
        if let Ok(cookie_str) = cookie_header.to_str() {
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
    }

    let cookie = "session_id=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT";
    axum::response::Response::builder()
        .status(axum::http::StatusCode::SEE_OTHER)
        .header(axum::http::header::LOCATION, "/login")
        .header(axum::http::header::SET_COOKIE, cookie)
        .body(axum::body::Body::empty())
        .unwrap()
}
