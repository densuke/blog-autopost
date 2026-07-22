pub mod routes;

use crate::config::Config;
use crate::scheduled::{JsonScheduledPostStore, ScheduledPostExecutor};
use crate::sns::{
    bluesky::BlueskyClient, mastodon::MastodonClient, misskey::MisskeyClient, traits::SnsClient,
    x::XClient,
};
use crate::timing::TimingManager;
use axum::{
    Router,
    extract::{DefaultBodyLimit, State},
    routing::{get, post, put},
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use std::collections::HashMap;

// アプリケーション全体で共有する状態
pub struct AppState {
    pub config: Config,
    pub timing_manager: TimingManager,
    pub store: Arc<JsonScheduledPostStore>,
    pub config_path: String,
    pub sessions: Arc<tokio::sync::RwLock<HashMap<String, String>>>,
    pub mcp_sessions: Arc<
        tokio::sync::RwLock<HashMap<String, tokio::sync::mpsc::Sender<axum::response::sse::Event>>>,
    >,
}

pub async fn start_server(config: Config, config_path: String, port: u16) -> anyhow::Result<()> {
    let timing_manager = TimingManager::new(&config);
    let store = Arc::new(JsonScheduledPostStore::new("data/scheduled_posts.json"));

    // SnsClient のリストを生成 (予約投稿のバックグラウンド実行用)
    let mut sns_clients: Vec<Arc<dyn SnsClient + Send + Sync>> = Vec::new();
    for sns_conf in &config.sns {
        match sns_conf {
            crate::config::SnsConfig::Mastodon {
                instance_url,
                access_token,
                name,
                ..
            } => {
                if let Ok(client) =
                    MastodonClient::new(instance_url.clone(), access_token.clone(), name.clone())
                {
                    sns_clients.push(Arc::new(client));
                }
            }
            crate::config::SnsConfig::Misskey {
                instance_url,
                access_token,
                name,
                ..
            } => {
                if let Ok(client) =
                    MisskeyClient::new(instance_url.clone(), access_token.clone(), name.clone())
                {
                    sns_clients.push(Arc::new(client));
                }
            }
            crate::config::SnsConfig::Bluesky {
                identifier,
                password,
                name,
                ..
            } => {
                if let Ok(client) =
                    BlueskyClient::new(identifier.clone(), password.clone(), name.clone())
                {
                    sns_clients.push(Arc::new(client));
                }
            }
            crate::config::SnsConfig::X {
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
    let mcp_sessions = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
    let state = Arc::new(AppState {
        config: config.clone(),
        timing_manager,
        store,
        config_path,
        sessions,
        mcp_sessions,
    });

    // CORS設定
    let cors = CorsLayer::permissive();

    // ルーティング設定
    let api_routes = Router::new()
        .route("/config", get(routes::get_config))
        .route("/post", post(routes::manual_post))
        .route("/upload", post(routes::upload_media))
        .route("/next-slots", get(routes::get_next_slots))
        .route("/schedules", get(routes::get_schedules))
        .route(
            "/schedules/{id}",
            put(routes::update_schedule).delete(routes::delete_schedule),
        )
        .route("/schedules/{id}/post-now", post(routes::post_now_schedule))
        .route("/mcp/sse", get(routes::mcp_sse_handler))
        .route("/mcp/message", post(routes::mcp_message_handler))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let app = Router::new()
        .nest("/api", api_routes)
        .route(
            "/login",
            get(routes::get_login_page).post(routes::login_submit),
        )
        .route("/logout", get(routes::logout))
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    println!("Web UI listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

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

    // 1. APIキー (Bearerトークン or X-Api-Key) による認証のチェック
    if let Some(auth_header) = req.headers().get(axum::http::header::AUTHORIZATION)
        && let Ok(auth_str) = auth_header.to_str()
        && let Some(token) = auth_str.strip_prefix("Bearer ")
        && let Some(ref config_auth) = state.config.web_auth
        && let Some(ref secret) = config_auth.secret_key
        && token == secret
    {
        authenticated = true;
    }

    if !authenticated
        && let Some(api_key_header) = req.headers().get("X-Api-Key")
        && let Ok(api_key) = api_key_header.to_str()
        && let Some(ref config_auth) = state.config.web_auth
        && let Some(ref secret) = config_auth.secret_key
        && api_key == secret
    {
        authenticated = true;
    }

    // 2. Cookieセッションによる認証のチェック
    if !authenticated
        && let Some(cookie_header) = req.headers().get(axum::http::header::COOKIE)
        && let Ok(cookie_str) = cookie_header.to_str()
    {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WebAuthConfig;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::get,
    };
    use tower::ServiceExt; // for oneshot

    // ヘルパー: テスト用の AppState と Router を作成
    async fn setup_test_router(secret_key: Option<String>) -> (Router, Arc<AppState>) {
        let config = Config {
            announcement_text: None,
            blog: None,
            sns: vec![],
            templates: HashMap::new(),
            default_allowed_timings: None,
            allowed_timings_tolerance_minutes: None,
            allowed_timings: None,
            web_auth: Some(WebAuthConfig {
                username: "admin".to_string(),
                password: "hashed_password".to_string(),
                secret_key,
            }),
            extra: HashMap::new(),
        };

        let timing_manager = TimingManager::new(&config);
        let store = Arc::new(JsonScheduledPostStore::new(
            "data/test_scheduled_posts.json",
        ));
        let sessions = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        let mcp_sessions = Arc::new(tokio::sync::RwLock::new(HashMap::new()));

        let state = Arc::new(AppState {
            config,
            timing_manager,
            store,
            config_path: "config.yaml".to_string(),
            sessions,
            mcp_sessions,
        });

        // 認証付きAPIルートと未認証ルートを設定
        let api_routes = Router::new()
            .route("/config", get(|| async { "config data" }))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ));

        let app = Router::new()
            .nest("/api", api_routes)
            .route("/login", get(|| async { "login page" }))
            .fallback(|| async { "fallback page" })
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state.clone());

        (app, state)
    }

    #[tokio::test]
    async fn test_auth_middleware_no_auth_api() {
        let (app, _) = setup_test_router(Some("my-secret-token".to_string())).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_middleware_no_auth_web_redirect() {
        let (app, _) = setup_test_router(Some("my-secret-token".to_string())).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/some-page")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::LOCATION)
                .unwrap(),
            "/login"
        );
    }

    #[tokio::test]
    async fn test_auth_middleware_bearer_token_success() {
        let (app, _) = setup_test_router(Some("my-secret-token".to_string())).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/config")
                    .header(axum::http::header::AUTHORIZATION, "Bearer my-secret-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_middleware_x_api_key_success() {
        let (app, _) = setup_test_router(Some("my-secret-token".to_string())).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/config")
                    .header("X-Api-Key", "my-secret-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_middleware_cookie_session_success() {
        let (app, state) = setup_test_router(Some("my-secret-token".to_string())).await;

        // セッションを登録
        {
            let mut sessions = state.sessions.write().await;
            sessions.insert("my-session-id".to_string(), "admin".to_string());
        }

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/config")
                    .header(axum::http::header::COOKIE, "session_id=my-session-id")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
