use std::convert::Infallible;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use futures::stream::Stream;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;

use super::server::{AppState, LimitQuery, mime_for, UI_DIR};

pub async fn prompts_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(q): Query<LimitQuery>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let limit = q.limit;
    match tokio::task::spawn_blocking(move || db.query_user_prompts(&session_id, limit)).await {
        Ok(Ok(rows)) => Json(rows).into_response(),
        Ok(Err(e)) => {
            tracing::error!("prompts query: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(e) => {
            tracing::error!("spawn_blocking panic: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn sessions_handler(
    State(state): State<AppState>,
    Query(q): Query<LimitQuery>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let limit = q.limit;
    match tokio::task::spawn_blocking(move || db.query_sessions(limit)).await {
        Ok(Ok(rows)) => Json(rows).into_response(),
        Ok(Err(e)) => {
            tracing::error!("sessions query: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(e) => {
            tracing::error!("spawn_blocking panic: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn events_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(q): Query<LimitQuery>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let limit = q.limit;
    match tokio::task::spawn_blocking(move || db.query_events(&session_id, limit)).await {
        Ok(Ok(rows)) => Json(rows).into_response(),
        Ok(Err(e)) => {
            tracing::error!("events query: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(e) => {
            tracing::error!("spawn_blocking panic: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn files_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(q): Query<LimitQuery>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let limit = q.limit;
    match tokio::task::spawn_blocking(move || db.query_file_accesses(&session_id, limit)).await {
        Ok(Ok(rows)) => Json(rows).into_response(),
        Ok(Err(e)) => {
            tracing::error!("files query: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(e) => {
            tracing::error!("spawn_blocking panic: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(|result| result.ok())
        .map(|value| {
            let data = value.to_string();
            Ok::<_, Infallible>(SseEvent::default().data(data))
        });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub async fn static_handler(req: axum::extract::Request) -> impl IntoResponse {
    let uri_path = req.uri().path().trim_start_matches('/');
    let file_path = if uri_path.is_empty() { "index.html" } else { uri_path };

    if let Some(file) = UI_DIR.get_file(file_path) {
        let mime = mime_for(file_path);
        (
            [(header::CONTENT_TYPE, HeaderValue::from_static(mime))],
            file.contents(),
        )
            .into_response()
    } else {
        if let Some(index) = UI_DIR.get_file("index.html") {
            (
                [(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))],
                index.contents(),
            )
                .into_response()
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    }
}
