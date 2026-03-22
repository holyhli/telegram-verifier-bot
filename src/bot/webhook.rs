use axum::body::Bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub mode: &'static str,
}

pub fn health_router(mode: &'static str) -> Router {
    Router::new().route(
        "/health",
        get(move || async move {
            Json(HealthResponse {
                status: "ok",
                mode,
            })
        }),
    )
}

pub fn parse_telegram_update(
    body: &[u8],
) -> Result<teloxide::types::Update, serde_json::Error> {
    serde_json::from_slice(body)
}

pub async fn webhook_update_handler(body: Bytes) -> axum::response::Response {
    match parse_telegram_update(&body) {
        Ok(update) => {
            tracing::debug!(update_id = update.id.0, "received webhook update");
            StatusCode::OK.into_response()
        }
        Err(err) => {
            tracing::warn!(error = %err, "invalid webhook update JSON");
            (StatusCode::BAD_REQUEST, "invalid update JSON").into_response()
        }
    }
}

pub fn webhook_app(mode: &'static str) -> Router {
    health_router(mode).route("/webhook", post(webhook_update_handler))
}
