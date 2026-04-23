use std::sync::Arc;

use anyhow::Result;
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use chrono::Utc;
use serde_json::Value;
use tokio::sync::broadcast;

use crate::masker;
use crate::storage::{Event, FileAccess, StorageHandle, UserPrompt};

#[derive(Clone)]
struct AppState {
    tx: Arc<broadcast::Sender<Value>>,
    db: Arc<StorageHandle>,
}

pub async fn serve(tx: Arc<broadcast::Sender<Value>>, db: Arc<StorageHandle>) -> Result<()> {
    let state = AppState { tx, db };
    let app = Router::new()
        .route("/hook", post(handle_hook))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:7421").await?;
    tracing::info!("hook receiver listening on 127.0.0.1:7421");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_hook(
    State(state): State<AppState>,
    Json(mut payload): Json<Value>,
) -> (StatusCode, Json<Value>) {

    let ts = Utc::now().timestamp_millis();
    let session_id = payload
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let event_type = payload
        .get("hook_event_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    if event_type == "UserPromptSubmit" {
        println!("Payload{}", payload);
        let original = payload
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let masked = payload
            .get("prompt")
            .and_then(|v| v.as_str())
            .map(|s| {
                let mut tmp = serde_json::Value::String(s.to_string());
                masker::mask(&mut tmp);
                tmp.as_str().unwrap_or(s).to_string()
            })
            .unwrap_or_default();

        if !original.is_empty() {
            let up = UserPrompt { session_id: session_id.clone(), prompt: masked.clone(), ts };
            state.db.write_user_prompt(up).await;
        }

        if masked != original {
            let reason = format!("Sensitive data detected and blocked. Redacted prompt: {masked}");
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "decision": "block", "reason": reason })),
            );
        }
    }

    if event_type == "PreToolUse" || event_type == "PostToolUse" {
        if let Some(tool_name) = payload.get("tool_name").and_then(|v| v.as_str()) {
            if matches!(tool_name, "Read" | "Write" | "Edit" | "Bash") {
                let path = payload
                    .pointer("/tool_input/file_path")
                    .or_else(|| payload.pointer("/tool_input/path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if !path.is_empty() {
                    let fa = FileAccess {
                        session_id: session_id.clone(),
                        path,
                        operation: tool_name.to_string(),
                        ts,
                    };
                    state.db.write_file_access(fa).await;
                }
            }
        }
    }

    let ev = Event {
        session_id,
        event_type,
        payload: payload.clone(),
        ts,
    };
    state.db.write_event(ev).await;

    let _ = state.tx.send(payload);

    (StatusCode::OK, Json(serde_json::json!({})))
}
