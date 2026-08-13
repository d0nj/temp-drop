mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use common::*;

async fn raw_body(resp: axum::http::Response<axum::body::Body>) -> Vec<u8> {
    axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

#[tokio::test]
async fn full_download_serves_bytes_and_headers() {
    let env = TestEnv::new().await;
    let id = seed_ready_file(&env, "doc.pdf", b"0123456789").await;
    let resp = env
        .request(
            Request::builder()
                .uri(format!("/raw/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap(),
        r#"attachment; filename="doc.pdf"; filename*=UTF-8''doc.pdf"#
    );
    assert_eq!(resp.headers()["accept-ranges"], "bytes");
    assert_eq!(resp.headers()["cache-control"], "private, no-store");
    assert_eq!(resp.headers()["content-length"], "10");
    assert_eq!(raw_body(resp).await, b"0123456789");
}

#[tokio::test]
async fn single_range_returns_206() {
    let env = TestEnv::new().await;
    let id = seed_ready_file(&env, "r.bin", b"0123456789").await;
    let resp = env
        .request(
            Request::builder()
                .uri(format!("/raw/{id}"))
                .header(header::RANGE, "bytes=2-5")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(resp.headers()["content-range"], "bytes 2-5/10");
    assert_eq!(resp.headers()["content-length"], "4");
    assert_eq!(raw_body(resp).await, b"2345");
}

#[tokio::test]
async fn suffix_range_returns_206() {
    let env = TestEnv::new().await;
    let id = seed_ready_file(&env, "s.bin", b"0123456789").await;
    let resp = env
        .request(
            Request::builder()
                .uri(format!("/raw/{id}"))
                .header(header::RANGE, "bytes=-3")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(resp.headers()["content-range"], "bytes 7-9/10");
    assert_eq!(raw_body(resp).await, b"789");
}

#[tokio::test]
async fn malformed_or_multi_range_returns_full_200() {
    let env = TestEnv::new().await;
    let id = seed_ready_file(&env, "m.bin", b"0123456789").await;
    for bad in ["bytes=banana", "bytes=0-1,4-5", ""] {
        let mut req = Request::builder().uri(format!("/raw/{id}"));
        if !bad.is_empty() {
            req = req.header(header::RANGE, bad);
        }
        let resp = env.request(req.body(Body::empty()).unwrap()).await;
        assert_eq!(resp.status(), StatusCode::OK, "header {bad:?}");
    }
}

#[tokio::test]
async fn download_consumes_and_depletes_count() {
    let env = TestEnv::new().await;
    let v = start_limited(&env, "ltd.bin", 2).await;
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
    assert_eq!(
        env.request(
            Request::builder()
                .uri(format!("/raw/{id}"))
                .body(Body::empty())
                .unwrap()
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        env.request(
            Request::builder()
                .uri(format!("/raw/{id}"))
                .body(Body::empty())
                .unwrap()
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        env.request(
            Request::builder()
                .uri(format!("/raw/{id}"))
                .body(Body::empty())
                .unwrap()
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        env.request(
            Request::builder()
                .uri(format!("/api/uploads/{id}"))
                .body(Body::empty())
                .unwrap()
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn unknown_id_is_404() {
    let env = TestEnv::new().await;
    let resp = env
        .request(
            Request::builder()
                .uri("/raw/nope123")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
