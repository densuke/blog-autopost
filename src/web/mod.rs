pub mod routes;

use std::sync::Arc;
use axum::{
    routing::{get, post, put},
    Router,
};
use tower_http::services::ServeDir;
use tower_http::cors::CorsLayer;
use crate::config::Config;
use crate::timing::TimingManager;
use crate::scheduled::{JsonScheduledPostStore, ScheduledPostExecutor};
use crate::sns::{
    mastodon::MastodonClient, misskey::MisskeyClient, bluesky::BlueskyClient, x::XClient, traits::SnsClient,
};

// アプリケーション全体で共有する状態
pub struct AppState {
    pub config: Config,
    pub timing_manager: TimingManager,
    pub store: Arc<JsonScheduledPostStore>,
}

pub async fn start_server(config: Config, port: u16) -> anyhow::Result<()> {
    let timing_manager = TimingManager::new(&config);
    let store = Arc::new(JsonScheduledPostStore::new("data/scheduled_posts.json"));

    // SnsClient のリストを生成 (予約投稿のバックグラウンド実行用)
    let mut sns_clients: Vec<Arc<dyn SnsClient + Send + Sync>> = Vec::new();
    for sns_conf in &config.sns {
        match sns_conf {
            crate::config::SnsConfig::Mastodon { instance_url, access_token, name, .. } => {
                if let Ok(client) = MastodonClient::new(instance_url.clone(), access_token.clone(), name.clone()) {
                    sns_clients.push(Arc::new(client));
                }
            }
            crate::config::SnsConfig::Misskey { instance_url, access_token, name, .. } => {
                if let Ok(client) = MisskeyClient::new(instance_url.clone(), access_token.clone(), name.clone()) {
                    sns_clients.push(Arc::new(client));
                }
            }
            crate::config::SnsConfig::Bluesky { identifier, password, name, .. } => {
                if let Ok(client) = BlueskyClient::new(identifier.clone(), password.clone(), name.clone()) {
                    sns_clients.push(Arc::new(client));
                }
            }
            crate::config::SnsConfig::X { consumer_key, consumer_secret, access_token, access_token_secret, name } => {
                if let Ok(client) = XClient::new(consumer_key.clone(), consumer_secret.clone(), access_token.clone(), access_token_secret.clone(), name.clone()) {
                    sns_clients.push(Arc::new(client));
                }
            }
            _ => {}
        }
    }

    // 予約投稿のバックグラウンド実行ループを起動 (30秒間隔)
    let executor = Arc::new(ScheduledPostExecutor::new(
        store.clone(),
        sns_clients,
        false, // dry_run
    ));

    let executor_clone = executor.clone();
    tokio::spawn(async move {
        println!("Background scheduled post executor started.");
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            if let Err(e) = executor_clone.execute_pending_posts().await {
                println!("Error executing pending posts in background: {:?}", e);
            }
        }
    });

    let state = Arc::new(AppState { config: config.clone(), timing_manager, store });

    // CORS設定
    let cors = CorsLayer::permissive();

    // ルーティング設定
    let api_routes = Router::new()
        .route("/config", get(routes::get_config))
        .route("/post", post(routes::manual_post))
        .route("/next-slots", get(routes::get_next_slots))
        .route("/schedules", get(routes::get_schedules))
        .route("/schedules/{id}", put(routes::update_schedule).delete(routes::delete_schedule))
        .with_state(state);

    let app = Router::new()
        .nest("/api", api_routes)
        // ルートからアクセスされた場合はstatic以下を配信する
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .layer(cors);

    let addr = format!("0.0.0.0:{}", port);
    println!("Web UI listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
