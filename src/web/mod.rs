pub mod routes;

use crate::config::Config;
use crate::scheduled::{JsonScheduledPostStore, ScheduledPostExecutor};
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

/// アプリケーションのルータを構築する。
///
/// サーバの起動処理(ポートのバインドと待ち受け)とは分離してあり、
/// テストから `tower::ServiceExt::oneshot` を使って本物のハンドラを
/// 直接検証できるようにしている。
pub fn build_router(state: Arc<AppState>) -> Router {
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

    Router::new()
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
        .with_state(state)
}

pub async fn start_server(config: Config, config_path: String, port: u16) -> anyhow::Result<()> {
    let timing_manager = TimingManager::new(&config);
    let store = Arc::new(JsonScheduledPostStore::new("data/scheduled_posts.json"));

    // SnsClient のリストを生成 (予約投稿のバックグラウンド実行用)
    let sns_clients = crate::sns::build_clients_from_config(&config);

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

    let app = build_router(state);

    let addr = format!("0.0.0.0:{}", port);
    println!("Web UI listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// 認証なしでアクセスできるパスかどうかを判定する。
///
/// ログイン画面自体と、そこから読み込まれるテーマ関連の共有アセットを許可する。
///
/// `static/` はルータの `ServeDir` がルート直下へ配信するため `/static/...` という
/// URL は存在しない。意図しないパスを開けないよう完全一致のみで判定する。
fn is_public_path(path: &str) -> bool {
    path == "/login" || path == "/theme.js" || path == "/theme.css"
}

// 認証確認ミドルウェア
async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, axum::http::StatusCode> {
    let path = req.uri().path();

    if is_public_path(path) {
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
    };
    use tower::ServiceExt; // for oneshot

    #[test]
    fn test_is_public_path() {
        // ログイン画面と、そこから読み込む共有アセットは認証不要
        assert!(is_public_path("/login"));
        assert!(is_public_path("/theme.js"));
        assert!(is_public_path("/theme.css"));

        // それ以外は認証が必要
        assert!(!is_public_path("/"));
        assert!(!is_public_path("/index.html"));
        assert!(!is_public_path("/api/config"));

        // ServeDir はルート直下へ配信するため /static/... は存在しない。
        // 前方一致で余計なパスを開けていないことを確認する
        assert!(!is_public_path("/static/app.css"));
        assert!(!is_public_path("/theme.js.map"));
        assert!(!is_public_path("/login/extra"));
    }

    pub(crate) const TEST_USERNAME: &str = "admin";
    pub(crate) const TEST_PASSWORD: &str = "password";

    /// テスト用の一時作業領域とアプリケーション状態。
    ///
    /// `TempDir` はドロップ時に実体ごと削除されるため、テスト終了まで
    /// 保持し続ける必要がある。
    pub(crate) struct TestApp {
        _dir: tempfile::TempDir,
        pub state: Arc<AppState>,
        pub router: Router,
    }

    /// 本物のルータを備えたテスト環境を作る。
    ///
    /// ダミーハンドラではなく `build_router` を使うため、`routes` 配下の
    /// 実装がそのまま検証対象になる。予約投稿の保存先は一時ディレクトリへ
    /// 向けており、実際の `data/` には触れない。
    pub(crate) fn setup_test_app(secret_key: Option<String>) -> TestApp {
        setup_test_app_with_config(secret_key, |_| {})
    }

    /// 設定を調整できる版のテスト環境。
    pub(crate) fn setup_test_app_with_config(
        secret_key: Option<String>,
        customize: impl FnOnce(&mut Config),
    ) -> TestApp {
        let dir = tempfile::TempDir::new().expect("一時ディレクトリの作成に失敗");

        let mut config = Config {
            announcement_text: None,
            blog: None,
            sns: vec![],
            templates: HashMap::new(),
            default_allowed_timings: None,
            allowed_timings_tolerance_minutes: None,
            allowed_timings: None,
            web_auth: Some(WebAuthConfig {
                username: TEST_USERNAME.to_string(),
                // テストの実行時間を抑えるためコストを最低にしてハッシュ化する
                password: bcrypt::hash(TEST_PASSWORD, 4).expect("ハッシュ化に失敗"),
                secret_key,
            }),
            extra: HashMap::new(),
        };
        customize(&mut config);

        let timing_manager = TimingManager::new(&config);
        let store = Arc::new(JsonScheduledPostStore::new(
            dir.path().join("scheduled_posts.json"),
        ));
        let sessions = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        let mcp_sessions = Arc::new(tokio::sync::RwLock::new(HashMap::new()));

        let state = Arc::new(AppState {
            config,
            timing_manager,
            store,
            config_path: dir.path().join("config.yml").to_string_lossy().into_owned(),
            sessions,
            mcp_sessions,
        });

        let router = build_router(state.clone());

        TestApp {
            _dir: dir,
            state,
            router,
        }
    }

    /// 旧来の呼び出し形に合わせたヘルパー。
    async fn setup_test_router(secret_key: Option<String>) -> (Router, Arc<AppState>) {
        let app = setup_test_app(secret_key);
        // TestApp をここで落とすと一時ディレクトリが消えるため、
        // 使い捨ての用途に限定する(認証ミドルウェアの検証のみ)。
        let TestApp { state, router, .. } = app;
        (router, state)
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
