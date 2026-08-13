//! Requires a running MinIO (docker compose in CI): TEMPDROP_TEST_S3=1
//! TEMPDROP_S3_ENDPOINT=http://127.0.0.1:9000 TEMPDROP_S3_BUCKET=tempdrop-test
//! TEMPDROP_S3_ACCESS_KEY=minioadmin TEMPDROP_S3_SECRET_KEY=minioadmin

mod common;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use common::TestEnv;
use serde_json::{json, Value};
use tempdrop::config::Config;
use tempdrop::db::Db;
use tempdrop::rate_limit::RateLimiter;
use tempdrop::state::AppState;
use tempdrop::storage::s3::S3Storage;
use tempdrop::storage::Storage;
use tempfile::TempDir;

fn s3_config() -> Option<Config> {
    if std::env::var("TEMPDROP_TEST_S3").as_deref() != Ok("1") {
        return None;
    }
    let mut c = Config::default();
    c.storage.backend = "s3".into();
    c.storage.s3.endpoint =
        std::env::var("TEMPDROP_S3_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:9000".into());
    c.storage.s3.bucket =
        std::env::var("TEMPDROP_S3_BUCKET").unwrap_or_else(|_| "tempdrop-test".into());
    c.storage.s3.access_key =
        std::env::var("TEMPDROP_S3_ACCESS_KEY").unwrap_or_else(|_| "minioadmin".into());
    c.storage.s3.secret_key =
        std::env::var("TEMPDROP_S3_SECRET_KEY").unwrap_or_else(|_| "minioadmin".into());
    c.storage.s3.force_path_style = true;
    Some(c)
}

async fn s3_env() -> Option<TestEnv> {
    let cfg = s3_config()?;
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    let s3 = S3Storage::new(&cfg.storage.s3).await.unwrap();
    let state = AppState::new(db, Storage::S3(s3), cfg, RateLimiter::new(1000));
    Some(TestEnv { dir, state })
}

#[tokio::test]
async fn s3_full_upload_download_flow() {
    let Some(env) = s3_env().await else {
        eprintln!("skipped: TEMPDROP_TEST_S3 not set");
        return;
    };
    // start
    let resp = env
        .request(
            Request::builder()
                .method("POST")
                .uri("/api/uploads")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"name":"s3.bin","ttl_seconds":3600,"size_bytes":10}).to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let v: Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(v["backend"], "s3");
    let id = v["id"].as_str().unwrap().to_string();
    let token = v["upload_token"].as_str().unwrap().to_string();

    // presign part 1 and upload directly via plain HTTP
    let resp = env
        .request(
            Request::builder()
                .uri(format!("/api/uploads/{id}/parts/1/presign"))
                .header("X-Upload-Token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let pv: Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let url = pv["url"].as_str().unwrap().to_string();
    let http = reqwest::Client::new();
    let up = http
        .put(&url)
        .body(vec![1u8, 2, 3, 4, 5])
        .send()
        .await
        .unwrap();
    assert!(
        up.status().is_success(),
        "presigned PUT failed: {}",
        up.status()
    );
    let etag = up.headers()["etag"].to_str().unwrap().to_string();

    // part 2 → etag
    let resp = env
        .request(
            Request::builder()
                .uri(format!("/api/uploads/{id}/parts/2/presign"))
                .header("X-Upload-Token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    let pv: Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let up = http
        .put(pv["url"].as_str().unwrap())
        .body(vec![6u8, 7, 8, 9, 10])
        .send()
        .await
        .unwrap();
    let etag2 = up.headers()["etag"].to_str().unwrap().to_string();

    // complete
    let resp = env
        .request(
            Request::builder()
                .method("POST")
                .uri(format!("/api/uploads/{id}/complete"))
                .header("X-Upload-Token", &token)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"etags":[etag, etag2]}).to_string()))
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::OK, "complete failed");

    // metadata size from HeadObject
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
    assert_eq!(mv["size"], 10);

    // ranged download through the API
    let resp = env
        .request(
            Request::builder()
                .uri(format!("/raw/{id}"))
                .header(header::RANGE, "bytes=2-4")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    assert_eq!(bytes, vec![3u8, 4, 5]);
}

#[tokio::test]
async fn s3_abort_removes_multipart() {
    let Some(env) = s3_env().await else {
        return;
    };
    let resp = env
        .request(
            Request::builder()
                .method("POST")
                .uri("/api/uploads")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"name":"abort.bin","ttl_seconds":60}).to_string(),
                ))
                .unwrap(),
        )
        .await;
    let v: Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let id = v["id"].as_str().unwrap().to_string();
    let token = v["upload_token"].as_str().unwrap().to_string();
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
async fn s3_rejects_etag_count_mismatch() {
    let Some(env) = s3_env().await else {
        return;
    };
    let resp = env
        .request(
            Request::builder()
                .method("POST")
                .uri("/api/uploads")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"name":"m.bin","ttl_seconds":60}).to_string(),
                ))
                .unwrap(),
        )
        .await;
    let v: Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let id = v["id"].as_str().unwrap().to_string();
    let token = v["upload_token"].as_str().unwrap().to_string();
    let _ = env
        .request(
            Request::builder()
                .uri(format!("/api/uploads/{id}/parts/1/presign"))
                .header("X-Upload-Token", &token)
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    let resp = env
        .request(
            Request::builder()
                .method("POST")
                .uri(format!("/api/uploads/{id}/complete"))
                .header("X-Upload-Token", &token)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"etags":[]}).to_string()))
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}
