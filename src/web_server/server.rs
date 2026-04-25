
use std::sync::Arc;
use super::handlers::{sessions_handler, events_handler, files_handler, sse_handler, static_handler};

use anyhow::Result;
use axum::{http::Method, routing::get, Router};
use include_dir::{include_dir, Dir};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

use crate::storage::StorageHandle;

// UI Build
pub(super) static UI_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/ui/dist");

#[derive(Clone)]
pub(super) struct AppState {
    pub(super) tx: Arc<broadcast::Sender<Value>>,
    pub(super) db: Arc<StorageHandle>,
}

#[derive(Deserialize)]
pub(super) struct LimitQuery {
    #[serde(default = "default_limit")]
    pub(super) limit: usize,
}
pub(super) fn default_limit() -> usize {
    100
}

pub(super) fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript",
        "css" => "text/css",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        _ => "application/octet-stream",
    }
}

pub async fn serve(tx: Arc<broadcast::Sender<Value>>, db: Arc<StorageHandle>) -> Result<()> {
    let state = AppState { tx, db };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET])
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/sessions", get(sessions_handler))
        .route("/api/sessions/{session_id}/events", get(events_handler))
        .route("/api/sessions/{session_id}/files", get(files_handler))
        .route("/api/sessions/{session_id}/prompts", get(super::handlers::prompts_handler))
        .route("/events", get(sse_handler))
        .fallback(get(static_handler))
        .with_state(state)
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:7422").await?;
    tracing::info!("web server listening on http://127.0.0.1:7422");
    axum::serve(listener, app).await?;
    Ok(())
}
