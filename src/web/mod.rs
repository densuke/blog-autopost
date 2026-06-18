pub mod routes;

use std::sync::Arc;
use axum::{
    routing::{get, post},
    Router,
};
use tower_http::services::ServeDir;
use tower_http::cors::CorsLayer;
use crate::config::Config;

// アプリケーション全体で共有する状態
pub struct AppState {
    pub config: Config,
    // 必要に応じて SNSクライアントのファクトリやDB接続などを持たせる
}

pub async fn start_server(config: Config, port: u16) -> anyhow::Result<()> {
    let state = Arc::new(AppState { config });

    // CORS設定
    let cors = CorsLayer::permissive();

    // ルーティング設定
    let api_routes = Router::new()
        .route("/config", get(routes::get_config))
        .route("/post", post(routes::manual_post))
        .with_state(state);

    let app = Router::new()
        .nest("/api", api_routes)
        // staticディレクトリ以下の静的ファイルを配信
        .fallback_service(ServeDir::new("static"))
        .layer(cors);

    let addr = format!("0.0.0.0:{}", port);
    println!("Web UI listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
