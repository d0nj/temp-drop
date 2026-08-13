mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn rate_limit_rejects_excess() {
    let env = TestEnv::with_config(|mut c| {
        c.rate_limit.per_min = 3;
        c
    })
    .await;
    for _ in 0..3 {
        let resp = env
            .request(
                Request::builder()
                    .method("POST")
                    .uri("/api/uploads")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"name":"x","ttl_seconds":60}).to_string(),
                    ))
                    .unwrap(),
            )
            .await;
        assert_eq!(resp.status(), StatusCode::CREATED);
    }
    let resp = env
        .request(
            Request::builder()
                .method("POST")
                .uri("/api/uploads")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name":"x","ttl_seconds":60}).to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn storage_low_rejects_part() {
    let low = Arc::new(AtomicBool::new(false));
    let low_clone = low.clone();
    let env = TestEnv::with_local_space(Box::new(move |_| {
        if low_clone.load(Ordering::SeqCst) {
            10
        } else {
            u64::MAX
        }
    }))
    .await;

    let v = start_json(&env, "low.bin").await;
    let id = v["id"].as_str().unwrap().to_string();
    let token = v["upload_token"].as_str().unwrap().to_string();

    low.store(true, Ordering::SeqCst);

    let resp = env
        .request(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/uploads/{id}/parts/1"))
                .header("X-Upload-Token", &token)
                .body(Body::from(vec![1u8]))
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::INSUFFICIENT_STORAGE);
}

#[tokio::test]
async fn in_flight_limit_rejects_start() {
    let env = TestEnv::with_config(|mut c| {
        c.uploads.max_in_flight_bytes = 100;
        c
    })
    .await;
    let v = start_json(&env, "a.bin").await;
    let id = v["id"].as_str().unwrap().to_string();
    let token = v["upload_token"].as_str().unwrap().to_string();
    let _ = env
        .request(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/uploads/{id}/parts/1"))
                .header("X-Upload-Token", &token)
                .body(Body::from(vec![1u8; 100]))
                .unwrap(),
        )
        .await;
    let resp = env
        .request(
            Request::builder()
                .method("POST")
                .uri("/api/uploads")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name":"b","ttl_seconds":60}).to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn filename_crlf_injection_stripped() {
    let env = TestEnv::new().await;
    let v = start_json(&env, "evil\r\nInjected: yes.bin").await; // name with CRLF
    let id = v["id"].as_str().unwrap().to_string();
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
                .uri(format!("/raw/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    let cd = resp.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .to_string();
    assert!(!cd.contains('\r') && !cd.contains('\n'));
}
