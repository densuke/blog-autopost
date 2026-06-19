use std::sync::Arc;
use axum::{
    extract::State,
    Json,
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
}
