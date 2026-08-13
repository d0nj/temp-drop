mod common;

use axum::body::Body;
use axum::http::Request;
use common::*;
use tempdrop::janitor::sweep_once;

#[tokio::test]
async fn sweep_removes_expired_and_count_dead() {
    let env = TestEnv::new().await;
    // create a ready file, then backdate its row
    let id = seed_ready_file(&env, "gone.bin", b"data").await;
    env.state.db.set_expires_at(&id, 1).await.unwrap();

    assert!(env.file_path(&id).exists());
    let n = sweep_once(&env.state, 2).await.unwrap();
    assert_eq!(n, 1);
    assert!(!env.file_path(&id).exists());
}

#[tokio::test]
async fn sweep_ignores_live_rows() {
    let env = TestEnv::new().await;
    let id = seed_ready_file(&env, "live.bin", b"x").await;
    let n = sweep_once(&env.state, chrono::Utc::now().timestamp())
        .await
        .unwrap();
    assert_eq!(n, 0);
    let resp = env
        .request(
            Request::builder()
                .uri(format!("/raw/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn sweep_removes_stale_pending_local() {
    let env = TestEnv::new().await;
    let v = start_json(&env, "stale.bin").await;
    let id = v["id"].as_str().unwrap().to_string();
    env.state.db.set_created_at(&id, 0).await.unwrap();

    assert!(env.file_path(&id).exists());
    let n = sweep_once(&env.state, 3_600 * 25).await.unwrap();
    assert_eq!(n, 1);
    assert!(!env.file_path(&id).exists());
}
