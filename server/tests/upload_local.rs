mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use common::*;
use serde_json::{json, Value};

#[tokio::test]
async fn start_valid_upload() {
    let env = TestEnv::new().await;
    let resp = start(
        &env,
        json!({"name": "notes.txt", "ttl_seconds": 3600, "size_bytes": 12}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["backend"], "local");
    assert_eq!(v["chunk_size"], 33_554_432);
    assert_eq!(v["id"].as_str().unwrap().len(), 12);
    assert_eq!(v["upload_token"].as_str().unwrap().len(), 64);
    assert!(v["expires_at"].as_i64().unwrap() > 0);
    assert!(v["max_downloads"].is_null());
    // blob file exists on disk, row is pending
    let id = v["id"].as_str().unwrap().to_string();
    assert!(env.file_path(&id).exists());
}

#[tokio::test]
async fn start_requires_expiry_choice() {
    let env = TestEnv::new().await;
    let resp = start(&env, json!({"name": "x"})).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err_code(resp).await, "no_expiry");
}

#[tokio::test]
async fn start_rejects_oversize_declared() {
    let env = TestEnv::with_config(|mut c| {
        c.uploads.max_upload_size_bytes = 100;
        c
    })
    .await;
    let resp = start(
        &env,
        json!({"name": "big", "size_bytes": 1000, "ttl_seconds": 60}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(err_code(resp).await, "upload_too_large");
}

#[tokio::test]
async fn start_rejects_empty_name() {
    let env = TestEnv::new().await;
    let resp = start(&env, json!({"name": "  \n ", "ttl_seconds": 60})).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err_code(resp).await, "validation");
}

#[tokio::test]
async fn local_full_upload_flow() {
    let env = TestEnv::new().await;
    // start
    let resp = start(&env, json!({"name": "pic.bin", "ttl_seconds": 3600})).await;
    let v: Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let id = v["id"].as_str().unwrap().to_string();
    let token = v["upload_token"].as_str().unwrap().to_string();

    // put two parts
    let part1 = env
        .request(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/uploads/{id}/parts/1"))
                .header("X-Upload-Token", &token)
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(vec![1u8, 2, 3, 4]))
                .unwrap(),
        )
        .await;
    assert_eq!(part1.status(), StatusCode::OK);

    let part2 = env
        .request(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/uploads/{id}/parts/2"))
                .header("X-Upload-Token", &token)
                .body(Body::from(vec![5u8, 6]))
                .unwrap(),
        )
        .await;
    assert_eq!(part2.status(), StatusCode::OK);

    // complete
    let done = env
        .request(
            Request::builder()
                .method("POST")
                .uri(format!("/api/uploads/{id}/complete"))
                .header("X-Upload-Token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(done.status(), StatusCode::OK);
    let dv: Value = serde_json::from_slice(
        &axum::body::to_bytes(done.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(dv["url"], format!("/f/{id}"));

    // disk state
    assert_eq!(
        std::fs::read(env.file_path(&id)).unwrap(),
        vec![1u8, 2, 3, 4, 5, 6]
    );
}

#[tokio::test]
async fn part_requires_token_and_order() {
    let env = TestEnv::new().await;
    let v = start_json(&env, "a.bin").await;
    let id = v["id"].as_str().unwrap().to_string();

    // no token
    let resp = env
        .request(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/uploads/{id}/parts/1"))
                .body(Body::from(vec![1u8]))
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // wrong token
    let resp = env
        .request(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/uploads/{id}/parts/1"))
                .header("X-Upload-Token", "deadbeef")
                .body(Body::from(vec![1u8]))
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // skip part 1, send part 2
    let token = v["upload_token"].as_str().unwrap().to_string();
    let resp = env
        .request(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/uploads/{id}/parts/2"))
                .header("X-Upload-Token", &token)
                .body(Body::from(vec![1u8]))
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(err_code(resp).await, "part_out_of_order");
}

#[tokio::test]
async fn part_exceeding_chunk_size_rejected() {
    let env = TestEnv::new().await;
    let v = start_json(&env, "big.bin").await;
    let id = v["id"].as_str().unwrap().to_string();
    let token = v["upload_token"].as_str().unwrap().to_string();
    let big = vec![0u8; env.state.config.chunk_size() + 1];
    let resp = env
        .request(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/uploads/{id}/parts/1"))
                .header("X-Upload-Token", &token)
                .body(Body::from(big))
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(err_code(resp).await, "part_too_large");
}

#[tokio::test]
async fn complete_rejects_empty_and_unknown() {
    let env = TestEnv::new().await;
    let v = start_json(&env, "e.bin").await;
    let id = v["id"].as_str().unwrap().to_string();
    let token = v["upload_token"].as_str().unwrap().to_string();
    let done = env
        .request(
            Request::builder()
                .method("POST")
                .uri(format!("/api/uploads/{id}/complete"))
                .header("X-Upload-Token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(done.status(), StatusCode::CONFLICT);
    assert_eq!(err_code(done).await, "not_complete");

    // unknown id
    let resp = env
        .request(
            Request::builder()
                .method("POST")
                .uri("/api/uploads/nonexistent/complete")
                .header("X-Upload-Token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
