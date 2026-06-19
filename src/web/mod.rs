pub mod routes;

use std::sync::Arc;
use axum::{
    routing::{get, post, put},
    extract::DefaultBodyLimit,
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

use std::collections::HashMap;

// アプリケーション全体で共有する状態
pub struct AppState {
    pub config: Config,
    pub timing_manager: TimingManager,
    pub store: Arc<JsonScheduledPostStore>,
    pub config_path: String,
    pub sessions: Arc<tokio::sync::RwLock<HashMap<String, String>>>,
}

pub async fn start_server(config: Config, config_path: String, port: u16) -> anyhow::Result<()> {
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

    let sessions = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
    let state = Arc::new(AppState { config: config.clone(), timing_manager, store, config_path, sessions });

    // CORS設定
    let cors = CorsLayer::permissive();

    // 認証確認ミドルウェア
    async fn auth_middleware(
        State(state): State<Arc<AppState>>,
        req: axum::http::Request<axum::body::Body>,
        next: axum::middleware::Next,
    ) -> Result<axum::response::Response, axum::http::StatusCode> {
        let path = req.uri().path();
        
        if path == "/login" || path.starts_with("/static/") {
            return Ok(next.run(req).await);
        }
        
        let mut authenticated = false;
        if let Some(cookie_header) = req.headers().get(axum::http::header::COOKIE) {
            if let Ok(cookie_str) = cookie_header.to_str() {
                for cookie in cookie_str.split(';') {
                    let parts: Vec<&str> = cookie.trim().split('=').collect();
                    if parts.len() == 2 && parts[0] == "session_id" {
                        let session_id = parts[1];
                        let sessions = state.sessions.read().await;
                        if sessions.contains_key(session_id) {
                            authenticated = true;
                            break;
                        }
                    }
                }
            }
        }
        
        if authenticated {
            Ok(next.run(req).await)
        } else {
            if path.starts_with("/api/") {
                Err(axum::http::StatusCode::UNAUTHORIZED)
            } else {
                let response = axum::response::Response::builder()
                    .status(axum::http::StatusCode::SEE_OTHER)
                    .header(axum::http::header::LOCATION, "/login")
                    .body(axum::body::Body::empty())
                    .unwrap();
                Ok(response)
            }
        }
    }

    use axum::extract::State;

    // ルーティング設定
    let api_routes = Router::new()
        .route("/config", get(routes::get_config))
        .route("/post", post(routes::manual_post))
        .route("/upload", post(routes::upload_media))
        .route("/next-slots", get(routes::get_next_slots))
        .route("/schedules", get(routes::get_schedules))
        .route("/schedules/{id}", put(routes::update_schedule).delete(routes::delete_schedule))
        .route("/schedules/{id}/post-now", post(routes::post_now_schedule))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware));

    let app = Router::new()
        .nest("/api", api_routes)
        .route("/login", get(routes::get_login_page).post(routes::login_submit))
        .route("/logout", get(routes::logout))
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    println!("Web UI listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
