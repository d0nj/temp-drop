mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use serde_json::Value;

#[tokio::test]
async fn abort_deletes_partial_and_row() {
    let env = TestEnv::new().await;
    let v = start_json(&env, "cancel.bin").await;
    let id = v["id"].as_str().unwrap().to_string();
    let token = v["upload_token"].as_str().unwrap().to_string();
    let _ = env
        .request(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/uploads/{id}/parts/1"))
                .header("X-Upload-Token", &token)
                .body(Body::from(vec![1u8, 2]))
                .unwrap(),
        )
        .await;
    assert!(env.file_path(&id).exists());

    let resp = env
        .request(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/uploads/{id}"))
                .header("X-Upload-Token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert!(!env.file_path(&id).exists());

    let meta = env
        .request(
            Request::builder()
                .uri(format!("/api/uploads/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(meta.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn abort_ready_upload_conflicts() {
    let env = TestEnv::new().await;
    let v = start_json(&env, "done.bin").await;
    let id = v["id"].as_str().unwrap().to_string();
    let token = v["upload_token"].as_str().unwrap().to_string();
    let _ = env
        .request(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/uploads/{id}/parts/1"))
                .header("X-Upload-Token", &token)
                .body(Body::from(vec![9u8]))
                .unwrap(),
        )
        .await;
    let _ = env
        .request(
            Request::builder()
                .method("POST")
                .uri(format!("/api/uploads/{id}/complete"))
                .header("X-Upload-Token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;

    let resp = env
        .request(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/uploads/{id}"))
                .header("X-Upload-Token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(err_code(resp).await, "already_complete");
}

#[tokio::test]
async fn meta_shows_expiry_fields_and_hides_pending() {
    let env = TestEnv::new().await;
    let v = start_json(&env, "meta.bin").await;
    let id = v["id"].as_str().unwrap().to_string();

    // pending row: 404 (not yet ready)
    let resp = env
        .request(
            Request::builder()
                .uri(format!("/api/uploads/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // complete
    let token = v["upload_token"].as_str().unwrap().to_string();
    let _ = env
        .request(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/uploads/{id}/parts/1"))
                .header("X-Upload-Token", &token)
                .body(Body::from(vec![1u8]))
                .unwrap(),
        )
        .await;
    let _ = env
        .request(
            Request::builder()
                .method("POST")
                .uri(format!("/api/uploads/{id}/complete"))
                .header("X-Upload-Token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;

    let resp = env
        .request(
            Request::builder()
                .uri(format!("/api/uploads/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    let mv: Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(mv["name"], "meta.bin");
    assert_eq!(mv["status"], "ready");
    assert!(mv["created_at"].as_i64().unwrap() > 0);
    assert!(mv["expires_at"].is_null() || mv["expires_at"].as_i64().unwrap() > 0);
    assert!(mv["downloads_left"].is_null());

    // second upload with max_downloads
    let v2 = start_limited(&env, "lim.bin", 3).await;
    let id2 = v2["id"].as_str().unwrap().to_string();
    let token2 = v2["upload_token"].as_str().unwrap().to_string();
    let _ = env
        .request(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/uploads/{id2}/parts/1"))
                .header("X-Upload-Token", &token2)
                .body(Body::from(vec![1u8]))
                .unwrap(),
        )
        .await;
    let _ = env
        .request(
            Request::builder()
                .method("POST")
                .uri(format!("/api/uploads/{id2}/complete"))
                .header("X-Upload-Token", &token2)
                .body(Body::empty())
                .unwrap(),
        )
        .await;

    let resp2 = env
        .request(
            Request::builder()
                .uri(format!("/api/uploads/{id2}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    let mv2: Value = serde_json::from_slice(
        &axum::body::to_bytes(resp2.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(mv2["downloads_left"].as_i64().unwrap(), 3);
}
