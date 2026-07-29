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

/// 稼働バージョンと、より新しい版が公開されているかを返す（Agy #408）。
///
/// 実際の更新は行わない（Agy #409 の担当）。まだ一度も確認できていない場合でも、
/// 稼働バージョンだけは必ず返す（画面を壊さないため）。
pub async fn get_version(
    State(state): State<Arc<AppState>>,
) -> Json<crate::version_check::VersionStatus> {
    Json(state.version_status.read().await.clone())
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

pub async fn upload_media(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, StatusCode> {
    let mut saved_paths = Vec::new();
    let upload_dir = &state.upload_dir;

    if let Err(e) = std::fs::create_dir_all(upload_dir) {
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

        let save_path = upload_dir.join(crate::web::media::unique_file_name(&file_name));

        if let Err(e) = std::fs::write(&save_path, &bytes) {
            println!("Failed to write file to {:?}: {:?}", save_path, e);
            return Ok(Json(UploadResponse {
                success: false,
                paths: Vec::new(),
                error: Some(format!("Failed to save file: {}", e)),
            }));
        }

        saved_paths.push(save_path.to_string_lossy().into_owned());
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

/// `POST /login` — ログインを受け付け、セッション Cookie を発行する。
///
/// `Secure` を付けるかどうかの判定に元リクエストの情報が要るため、
/// ヘッダと URI も受け取る。接続元アドレスはレート制限の鍵に使うが、
/// テストの `oneshot` では得られないので `Option` で受ける。
/// `Form` は本文を消費するので必ず最後に置く。
pub async fn login_submit(
    State(state): State<Arc<AppState>>,
    crate::web::ratelimit::PeerAddr(peer): crate::web::ratelimit::PeerAddr,
    headers: axum::http::HeaderMap,
    uri: axum::http::Uri,
    axum::Form(payload): axum::Form<LoginPayload>,
) -> axum::response::Response {
    let Some(ref auth) = state.config.web_auth else {
        println!("web_auth is not configured in config.yml");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let rate_key = crate::web::ratelimit::rate_limit_key(peer, &payload.username);

    if let Err(retry_after) = state.login_rate_limiter.check(&rate_key).await {
        println!("Too many login attempts for {}", rate_key);
        return axum::response::Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header(axum::http::header::RETRY_AFTER, retry_after.to_string())
            .body(axum::body::Body::from(
                "Too many login attempts. Please wait and try again.",
            ))
            .expect("固定のレスポンスなので組み立てに失敗しない");
    }

    let mut verified = false;
    let mut needs_hash_migration = false;

    // ユーザー名が違う場合もパスワード検証と同じ経路で失敗させ、
    // 失敗としてレート制限に数える
    if payload.username == auth.username {
        if auth.password.starts_with("$2b$")
            || auth.password.starts_with("$2a$")
            || auth.password.starts_with("$2y$")
        {
            if let Ok(ok) = bcrypt::verify(&payload.password, &auth.password) {
                verified = ok;
            }
        } else if payload.password == auth.password {
            verified = true;
            needs_hash_migration = true;
        }
    }

    if !verified {
        state.login_rate_limiter.record_failure(&rate_key).await;
        return StatusCode::UNAUTHORIZED.into_response();
    }

    state.login_rate_limiter.record_success(&rate_key).await;

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

    let Some(session_id) = crate::web::session::generate_session_id() else {
        println!("Failed to obtain randomness for session id");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let ttl_hours = auth.effective_session_ttl_hours();
    let session = crate::web::session::Session::new(payload.username, ttl_hours);

    {
        let mut sessions = state.sessions.write().await;
        // 溜まった期限切れをここでまとめて片付ける
        crate::web::session::purge_expired(&mut sessions, chrono::Utc::now());
        sessions.insert(session_id.clone(), session);
    }

    let secure = crate::web::session::CookieSecure::from_config(auth.cookie_secure.as_deref())
        .should_set(crate::web::session::is_https_request(&headers, &uri));
    let cookie = crate::web::session::build_session_cookie(&session_id, ttl_hours, secure);
    axum::response::Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(axum::http::header::LOCATION, "/")
        .header(axum::http::header::SET_COOKIE, cookie)
        .body(axum::body::Body::empty())
        .expect("固定のレスポンスなので組み立てに失敗しない")
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    req: axum::http::Request<axum::body::Body>,
) -> impl axum::response::IntoResponse {
    if let Some(cookie_header) = req.headers().get(axum::http::header::COOKIE)
        && let Ok(cookie_str) = cookie_header.to_str()
        && let Some(session_id) = crate::web::session::extract_session_id(cookie_str)
    {
        let mut sessions = state.sessions.write().await;
        sessions.remove(session_id);
    }

    let cookie = crate::web::session::build_expired_cookie();
    axum::response::Response::builder()
        .status(axum::http::StatusCode::SEE_OTHER)
        .header(axum::http::header::LOCATION, "/login")
        .header(axum::http::header::SET_COOKIE, cookie)
        .body(axum::body::Body::empty())
        .unwrap()
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

        // 未知の種別は一覧に含めない
        let sns = body["active_sns"].as_array().unwrap();
        assert_eq!(sns.len(), 2, "Unknown は除外されるはず: {:?}", sns);
        assert_eq!(sns[0], "Mastodon (mstdn-main)");
        assert_eq!(sns[1], "Bluesky (bsky-main)");

        // 構造化された一覧にも同じ2件が並ぶ
        let accounts = body["sns_accounts"].as_array().unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0]["name"], "mstdn-main");
        assert_eq!(accounts[0]["sns_type"], "mastodon");
        assert_eq!(accounts[1]["name"], "bsky-main");
        assert_eq!(accounts[1]["sns_type"], "bluesky");
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
            "target_sns": ["mstdn-main", "bsky-main"],
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
        assert_eq!(body["target_sns"][0], "mstdn-main");
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
            "target_sns": ["mstdn-main"],
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
            "target_sns": ["mstdn-main"],
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
        // セッションIDは CSPRNG 由来の 256 ビットを16進で表したもの
        let session_id = cookie
            .strip_prefix("session_id=")
            .and_then(|s| s.split(';').next())
            .expect("session_id が含まれること");
        assert_eq!(session_id.len(), 64, "実際の値: {}", cookie);
        assert!(session_id.chars().all(|c| c.is_ascii_hexdigit()));

        assert!(cookie.contains("HttpOnly"), "HttpOnly属性が必要");
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Max-Age=86400"), "既定TTLは24時間");
        // 素の HTTP なので Secure は付かない
        assert!(!cookie.contains("Secure"), "実際の値: {}", cookie);

        let sessions = app.state.sessions.read().await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions
                .get(session_id)
                .expect("発行したIDで引ける")
                .username,
            TEST_USERNAME
        );
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

    // --- POST /logout ---

    /// セッションを1件仕込んだテスト環境を作る。
    async fn app_with_session(session_id: &str) -> TestApp {
        let app = app_with_auth();
        {
            let mut sessions = app.state.sessions.write().await;
            sessions.insert(
                session_id.to_string(),
                crate::web::session::Session::new("admin".to_string(), 24),
            );
        }
        app
    }

    /// ログアウトするとセッションが破棄される。
    #[tokio::test]
    async fn test_logout_removes_session() {
        let app = app_with_session("my-session").await;

        let request = Request::builder()
            .method("POST")
            .uri("/logout")
            .header(header::COOKIE, "session_id=my-session")
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers().get(header::LOCATION).unwrap(), "/login");
        assert!(app.state.sessions.read().await.is_empty());

        // Cookie を即時に失効させる
        let cookie = set_cookie(&response);
        assert!(cookie.contains("Max-Age=0"), "実際の値: {}", cookie);
    }

    /// GET でのログアウトは受け付けない。
    #[tokio::test]
    async fn getでのログアウトは受け付けない() {
        let app = app_with_session("my-session").await;

        let request = Request::builder()
            .uri("/logout")
            .header(header::COOKIE, "session_id=my-session")
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        // GET で状態を変えられると <img src="/logout"> で強制ログアウトできてしまう
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            app.state.sessions.read().await.len(),
            1,
            "セッションは残ったままであること"
        );
    }

    // --- ログイン試行のレート制限 ---

    /// 誤ったパスワードでログインを試みる。
    fn failed_login_request() -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "username={}&password=wrong-password",
                TEST_USERNAME
            )))
            .unwrap()
    }

    /// 上限を超えて失敗すると 429 と Retry-After が返る。
    #[tokio::test]
    async fn 失敗が上限を超えると429になる() {
        let app = setup_test_app_with_config(None, |config| {
            if let Some(ref mut auth) = config.web_auth {
                auth.login_max_attempts = Some(3);
                auth.login_window_seconds = Some(60);
            }
        });

        for i in 0..3 {
            let response = app
                .router
                .clone()
                .oneshot(failed_login_request())
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{}回目は認証失敗として扱う",
                i + 1
            );
        }

        let response = app
            .router
            .clone()
            .oneshot(failed_login_request())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .expect("Retry-Afterが必要")
            .to_str()
            .unwrap()
            .parse::<u64>()
            .expect("秒数として解釈できる");
        assert!(
            retry_after > 0 && retry_after <= 60,
            "実際の値: {}",
            retry_after
        );
    }

    /// 制限中は正しいパスワードでも受け付けない。
    #[tokio::test]
    async fn 制限中は正しいパスワードでも拒否する() {
        let app = setup_test_app_with_config(None, |config| {
            if let Some(ref mut auth) = config.web_auth {
                auth.login_max_attempts = Some(1);
            }
        });

        app.router
            .clone()
            .oneshot(failed_login_request())
            .await
            .unwrap();

        let response = app
            .router
            .clone()
            .oneshot(login_request(None))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(
            app.state.sessions.read().await.is_empty(),
            "セッションは作られない"
        );
    }

    /// ログインが成功すると失敗の記録が消える。
    #[tokio::test]
    async fn 成功すると失敗の記録が消える() {
        let app = setup_test_app_with_config(None, |config| {
            if let Some(ref mut auth) = config.web_auth {
                auth.login_max_attempts = Some(3);
            }
        });

        // 上限に届く手前まで失敗させる
        for _ in 0..2 {
            app.router
                .clone()
                .oneshot(failed_login_request())
                .await
                .unwrap();
        }

        let response = app
            .router
            .clone()
            .oneshot(login_request(None))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);

        assert_eq!(
            app.state.login_rate_limiter.tracked_keys().await,
            0,
            "成功で記録が消えること"
        );

        // 再び上限まで失敗できる
        for _ in 0..2 {
            let r = app
                .router
                .clone()
                .oneshot(failed_login_request())
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        }
    }

    /// 窓が明ければ再びログインを試せる。
    #[tokio::test(start_paused = true)]
    async fn 窓が明ければ再び試せる() {
        let app = setup_test_app_with_config(None, |config| {
            if let Some(ref mut auth) = config.web_auth {
                auth.login_max_attempts = Some(1);
                auth.login_window_seconds = Some(60);
            }
        });

        app.router
            .clone()
            .oneshot(failed_login_request())
            .await
            .unwrap();
        let blocked = app
            .router
            .clone()
            .oneshot(login_request(None))
            .await
            .unwrap();
        assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);

        tokio::time::advance(std::time::Duration::from_secs(61)).await;

        let response = app
            .router
            .clone()
            .oneshot(login_request(None))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER, "窓が明けたら通る");
    }

    /// 誤ったユーザー名もレート制限の対象になる。
    #[tokio::test]
    async fn 誤ったユーザー名も制限の対象になる() {
        let app = setup_test_app_with_config(None, |config| {
            if let Some(ref mut auth) = config.web_auth {
                auth.login_max_attempts = Some(2);
            }
        });

        let wrong_user = || {
            Request::builder()
                .method("POST")
                .uri("/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("username=intruder&password=whatever"))
                .unwrap()
        };

        for _ in 0..2 {
            let r = app.router.clone().oneshot(wrong_user()).await.unwrap();
            assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        }

        let r = app.router.clone().oneshot(wrong_user()).await.unwrap();
        assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    /// 接続元が取れない場合はユーザー名ごとに数える。
    #[tokio::test]
    async fn 接続元不明ならユーザー名ごとに数える() {
        let app = setup_test_app_with_config(None, |config| {
            if let Some(ref mut auth) = config.web_auth {
                auth.login_max_attempts = Some(1);
            }
        });

        // oneshot には接続情報がないため username が鍵になる
        app.router
            .clone()
            .oneshot(failed_login_request())
            .await
            .unwrap();

        let other_user = Request::builder()
            .method("POST")
            .uri("/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("username=someone-else&password=whatever"))
            .unwrap();
        let r = app.router.clone().oneshot(other_user).await.unwrap();

        // 鍵が固定だと1人の失敗で全員が止まる。別ユーザーは影響を受けない
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }

    /// 設定の書き戻しでレート制限の項目も増えない。
    #[tokio::test]
    async fn 書き戻しでレート制限の項目も増えない() {
        let app = setup_test_app_with_config(None, |config| {
            if let Some(ref mut auth) = config.web_auth {
                auth.password = "plaintext-pass".to_string();
            }
        });

        let request = Request::builder()
            .method("POST")
            .uri("/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "username={}&password=plaintext-pass",
                TEST_USERNAME
            )))
            .unwrap();
        app.router.clone().oneshot(request).await.unwrap();

        let written = std::fs::read_to_string(&app.state.config_path)
            .expect("設定ファイルが書き出されているはず");

        assert!(
            !written.contains("login_max_attempts"),
            "実際の内容: {}",
            written
        );
        assert!(
            !written.contains("login_window_seconds"),
            "実際の内容: {}",
            written
        );
    }

    // --- セッションの有効期限と Cookie 属性 ---

    /// ログインフォームを組み立てる。
    fn login_request(extra_header: Option<(&str, &str)>) -> Request<Body> {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
        if let Some((name, value)) = extra_header {
            builder = builder.header(name, value);
        }
        builder
            .body(Body::from(format!(
                "username={}&password={}",
                TEST_USERNAME, TEST_PASSWORD
            )))
            .unwrap()
    }

    /// レスポンスから Set-Cookie の値を取り出す。
    fn set_cookie(response: &axum::response::Response) -> String {
        response
            .headers()
            .get(header::SET_COOKIE)
            .expect("Set-Cookieが必要")
            .to_str()
            .unwrap()
            .to_string()
    }

    /// ログインのたびに異なるセッションIDが払い出される。
    #[tokio::test]
    async fn ログインごとにセッションidが変わる() {
        let app = app_with_auth();

        let first = set_cookie(
            &app.router
                .clone()
                .oneshot(login_request(None))
                .await
                .unwrap(),
        );
        let second = set_cookie(
            &app.router
                .clone()
                .oneshot(login_request(None))
                .await
                .unwrap(),
        );

        assert_ne!(first, second, "同じIDを再利用してはいけない");
        assert_eq!(app.state.sessions.read().await.len(), 2);
    }

    /// 期限切れのセッションでは認証が通らない。
    #[tokio::test]
    async fn 期限切れセッションは拒否される() {
        let app = setup_test_app_with_config(None, |_| {});
        {
            let mut sessions = app.state.sessions.write().await;
            sessions.insert(
                "expired".to_string(),
                crate::web::session::Session::with_created_at(
                    "admin".to_string(),
                    1,
                    chrono::Utc::now() - chrono::Duration::hours(2),
                ),
            );
        }

        let request = Request::builder()
            .uri("/api/config")
            .header(header::COOKIE, "session_id=expired")
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    /// 期限切れのセッションは提示された時点で取り除かれる。
    #[tokio::test]
    async fn 期限切れセッションは提示時に消える() {
        let app = setup_test_app_with_config(None, |_| {});
        {
            let mut sessions = app.state.sessions.write().await;
            sessions.insert(
                "expired".to_string(),
                crate::web::session::Session::with_created_at(
                    "admin".to_string(),
                    1,
                    chrono::Utc::now() - chrono::Duration::hours(2),
                ),
            );
        }

        let request = Request::builder()
            .uri("/api/config")
            .header(header::COOKIE, "session_id=expired")
            .body(Body::empty())
            .unwrap();
        app.router.clone().oneshot(request).await.unwrap();

        assert!(
            app.state.sessions.read().await.is_empty(),
            "期限切れは残さない"
        );
    }

    /// 有効なセッションなら認証が通る。
    #[tokio::test]
    async fn 有効なセッションは認証を通る() {
        let app = setup_test_app_with_config(None, |_| {});
        {
            let mut sessions = app.state.sessions.write().await;
            sessions.insert(
                "live".to_string(),
                crate::web::session::Session::new("admin".to_string(), 24),
            );
        }

        let request = Request::builder()
            .uri("/api/config")
            .header(header::COOKIE, "session_id=live")
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    /// ログイン時に期限切れのセッションがまとめて片付けられる。
    #[tokio::test]
    async fn ログイン時に期限切れが掃除される() {
        let app = app_with_auth();
        {
            let mut sessions = app.state.sessions.write().await;
            sessions.insert(
                "expired".to_string(),
                crate::web::session::Session::with_created_at(
                    "admin".to_string(),
                    1,
                    chrono::Utc::now() - chrono::Duration::hours(2),
                ),
            );
        }

        app.router
            .clone()
            .oneshot(login_request(None))
            .await
            .unwrap();

        let sessions = app.state.sessions.read().await;
        assert!(!sessions.contains_key("expired"), "期限切れが残っている");
        assert_eq!(sessions.len(), 1, "新しく作った1件だけが残る");
    }

    /// TTL を設定するとその値が Max-Age に反映される。
    #[tokio::test]
    async fn ttl設定はmax_ageに反映される() {
        let app = setup_test_app_with_config(None, |config| {
            if let Some(ref mut auth) = config.web_auth {
                auth.session_ttl_hours = Some(2);
            }
        });

        let cookie = set_cookie(
            &app.router
                .clone()
                .oneshot(login_request(None))
                .await
                .unwrap(),
        );

        assert!(cookie.contains("Max-Age=7200"), "実際の値: {}", cookie);
    }

    /// TTL に 0 を指定しても既定値が使われる。
    #[tokio::test]
    async fn ttlが0なら既定値を使う() {
        let app = setup_test_app_with_config(None, |config| {
            if let Some(ref mut auth) = config.web_auth {
                auth.session_ttl_hours = Some(0);
            }
        });

        let cookie = set_cookie(
            &app.router
                .clone()
                .oneshot(login_request(None))
                .await
                .unwrap(),
        );

        // 0 を許すと発行直後に切れてしまうため既定の24時間へ倒す
        assert!(cookie.contains("Max-Age=86400"), "実際の値: {}", cookie);
    }

    /// X-Forwarded-Proto: https なら Secure が付く。
    #[tokio::test]
    async fn https経由ならsecureが付く() {
        let app = app_with_auth();

        let response = app
            .router
            .clone()
            .oneshot(login_request(Some(("X-Forwarded-Proto", "https"))))
            .await
            .unwrap();

        assert!(set_cookie(&response).contains("; Secure"));
    }

    /// 素の HTTP では Secure を付けない。
    #[tokio::test]
    async fn 素のhttpではsecureを付けない() {
        let app = app_with_auth();

        let response = app
            .router
            .clone()
            .oneshot(login_request(Some(("X-Forwarded-Proto", "http"))))
            .await
            .unwrap();

        // 無条件に付けると HTTP 運用の環境がログイン不能になる
        assert!(!set_cookie(&response).contains("Secure"));
    }

    /// cookie_secure: always なら HTTP でも Secure が付く。
    #[tokio::test]
    async fn cookie_secure_alwaysはhttpでも付く() {
        let app = setup_test_app_with_config(None, |config| {
            if let Some(ref mut auth) = config.web_auth {
                auth.cookie_secure = Some("always".to_string());
            }
        });

        let response = app
            .router
            .clone()
            .oneshot(login_request(None))
            .await
            .unwrap();

        assert!(set_cookie(&response).contains("; Secure"));
    }

    /// cookie_secure: never なら HTTPS でも Secure を付けない。
    #[tokio::test]
    async fn cookie_secure_neverはhttpsでも付かない() {
        let app = setup_test_app_with_config(None, |config| {
            if let Some(ref mut auth) = config.web_auth {
                auth.cookie_secure = Some("never".to_string());
            }
        });

        let response = app
            .router
            .clone()
            .oneshot(login_request(Some(("X-Forwarded-Proto", "https"))))
            .await
            .unwrap();

        assert!(!set_cookie(&response).contains("Secure"));
    }

    /// 設定の書き戻しで未設定の新項目が増えない。
    #[tokio::test]
    async fn 設定の書き戻しで未設定項目が増えない() {
        let app = setup_test_app_with_config(None, |config| {
            if let Some(ref mut auth) = config.web_auth {
                // 平文パスワードにして bcrypt 移行(=書き戻し)を起こす
                auth.password = "plaintext-pass".to_string();
            }
        });

        let request = Request::builder()
            .method("POST")
            .uri("/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "username={}&password=plaintext-pass",
                TEST_USERNAME
            )))
            .unwrap();
        app.router.clone().oneshot(request).await.unwrap();

        let written = std::fs::read_to_string(&app.state.config_path)
            .expect("設定ファイルが書き出されているはず");

        // skip_serializing_if が無いと null として現れてしまう
        assert!(
            !written.contains("session_ttl_hours"),
            "未設定の項目が書き足されている: {}",
            written
        );
        assert!(
            !written.contains("cookie_secure"),
            "未設定の項目が書き足されている: {}",
            written
        );
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

    /// main 側で追加されたテスト群。
    /// SNS名の解決に関するテスト群。
    mod resolve_sns {
        // 親モジュール(tests)ではなく routes 本体のスコープを取り込む
        use crate::config::SnsConfig;
        use crate::web::routes::*;

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
                    instance_url: "https://mstdn.example.com".to_string(),
                    access_token: "g".to_string(),
                },
                SnsConfig::Misskey {
                    name: "misskey-io".to_string(),
                    instance_url: "https://misskey.example.com".to_string(),
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
}
