mod common;

use common::TestEnv;
use axum::body::Body;
use axum::http::{Request, StatusCode};

#[tokio::test]
async fn healthz_returns_ok() {
    let env = TestEnv::new().await;
    let resp = env
        .request(Request::builder().uri("/healthz").body(Body::empty()).unwrap())
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn static_fallback_serves_index() {
    let env = TestEnv::new().await;
    let resp = env
        .request(Request::builder().uri("/f/abc123456789").body(Body::empty()).unwrap())
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("Content-Type").unwrap(),
        "text/html"
    );
}
