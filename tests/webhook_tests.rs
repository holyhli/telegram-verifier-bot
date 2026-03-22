use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use verifier_bot::bot::shutdown::ShutdownSignal;
use verifier_bot::bot::webhook;

#[tokio::test]
async fn webhook_health_endpoint_returns_200() {
    let app = webhook::webhook_app("webhook");

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["mode"], "webhook");
}

#[tokio::test]
async fn webhook_health_endpoint_returns_polling_mode() {
    let app = webhook::health_router("polling");

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["mode"], "polling");
}

#[tokio::test]
async fn webhook_endpoint_parses_valid_update_json() {
    let app = webhook::webhook_app("webhook");

    let update_json = serde_json::json!({
        "update_id": 12345,
        "message": {
            "message_id": 1,
            "date": 1_234_567_890,
            "chat": { "id": 123, "type": "private" }
        }
    });

    let request = Request::builder()
        .method("POST")
        .uri("/webhook")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&update_json).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn webhook_endpoint_rejects_invalid_json_with_400() {
    let app = webhook::webhook_app("webhook");

    let request = Request::builder()
        .method("POST")
        .uri("/webhook")
        .header("content-type", "application/json")
        .body(Body::from("this is not valid json"))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn webhook_shutdown_signal_triggers_and_observes() {
    let shutdown = ShutdownSignal::new();
    let mut waiter = shutdown.clone();

    assert!(!shutdown.is_shutdown());

    shutdown.shutdown();

    assert!(shutdown.is_shutdown());

    tokio::time::timeout(std::time::Duration::from_millis(100), waiter.wait())
        .await
        .expect("wait() should resolve immediately after shutdown()");
}
