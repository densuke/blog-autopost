use std::sync::Arc;
use std::collections::HashMap;
use axum::{
    extract::{State, Path},
    Json,
    http::StatusCode,
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

            let media_files = payload.image_url.clone().map(|url| vec![url]).unwrap_or_default();
            let post = ScheduledPost::new(
                payload.text.clone(),
                scheduled_time,
                media_files,
                vec![sns_name.clone()],
            );

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
