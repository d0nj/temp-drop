#![allow(dead_code, unused_imports)]

use tempdrop::config::Config;
use tempdrop::db::Db;
use tempdrop::rate_limit::RateLimiter;
use tempdrop::state::AppState;
use tempdrop::storage::local::LocalStorage;
use tempdrop::storage::Storage;
use tempfile::TempDir;

pub struct TestEnv {
    pub dir: TempDir,
    pub state: AppState,
}

impl TestEnv {
    pub async fn new() -> TestEnv {
        Self::with_config(|c| c).await
    }

    pub async fn with_config(modify: impl FnOnce(Config) -> Config) -> TestEnv {
        let dir = tempfile::tempdir().unwrap();
        let files = dir.path().join("files");
        std::fs::create_dir_all(&files).unwrap();
        let db = Db::open(&dir.path().join("t.db")).await.unwrap();
        let config = modify(Config::default());
        let storage = Storage::Local(LocalStorage::with_space_checker(
            files,
            config.uploads.min_free_bytes,
            Box::new(|_| u64::MAX),
        ));
        let limiter = RateLimiter::new(config.rate_limit.per_min);
        TestEnv {
            dir,
            state: AppState::new(db, storage, config, limiter),
        }
    }

    pub async fn with_local_space(
        checker: Box<dyn Fn(&std::path::Path) -> u64 + Send + Sync>,
    ) -> TestEnv {
        let dir = tempfile::tempdir().unwrap();
        let files = dir.path().join("files");
        std::fs::create_dir_all(&files).unwrap();
        let db = Db::open(&dir.path().join("t.db")).await.unwrap();
        let config = Config::default();
        let storage = Storage::Local(LocalStorage::with_space_checker(
            files,
            config.uploads.min_free_bytes,
            checker,
        ));
        let limiter = RateLimiter::new(config.rate_limit.per_min);
        TestEnv {
            dir,
            state: AppState::new(db, storage, config, limiter),
        }
    }

    pub async fn request(
        &self,
        req: axum::http::Request<axum::body::Body>,
    ) -> axum::http::Response<axum::body::Body> {
        use tower::ServiceExt;
        let mut req = req;
        req.extensions_mut().insert(axum::extract::ConnectInfo(
            "127.0.0.1:1".parse::<std::net::SocketAddr>().unwrap(),
        ));
        tempdrop::router(self.state.clone())
            .oneshot(req)
            .await
            .unwrap()
    }

    pub fn file_path(&self, key: &str) -> std::path::PathBuf {
        self.dir.path().join("files").join(key)
    }
}

/// Start an upload with a 1h TTL; returns the parsed StartRes JSON.
pub async fn start_json(env: &TestEnv, name: &str) -> serde_json::Value {
    let resp = start(env, serde_json::json!({"name": name, "ttl_seconds": 3600})).await;
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Start an upload with a download-count expiry.
pub async fn start_limited(env: &TestEnv, name: &str, max_downloads: i64) -> serde_json::Value {
    let resp = start(
        env,
        serde_json::json!({"name": name, "max_downloads": max_downloads}),
    )
    .await;
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// POST /api/uploads with an arbitrary body; returns the response.
pub async fn start(
    env: &TestEnv,
    body: serde_json::Value,
) -> axum::http::Response<axum::body::Body> {
    env.request(
        axum::http::Request::builder()
            .method("POST")
            .uri("/api/uploads")
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap(),
    )
    .await
}

/// Complete a ready file over the API; returns the upload id.
pub async fn seed_ready_file(env: &TestEnv, name: &str, data: &[u8]) -> String {
    let v = start_json(env, name).await;
    let id = v["id"].as_str().unwrap().to_string();
    let token = v["upload_token"].as_str().unwrap().to_string();
    for (n, chunk) in data.chunks(4).enumerate() {
        let _ = env
            .request(
                axum::http::Request::builder()
                    .method("PUT")
                    .uri(format!("/api/uploads/{id}/parts/{}", n + 1))
                    .header("X-Upload-Token", &token)
                    .body(axum::body::Body::from(chunk.to_vec()))
                    .unwrap(),
            )
            .await;
    }
    let _ = env
        .request(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/api/uploads/{id}/complete"))
                .header("X-Upload-Token", &token)
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await;
    id
}

/// Parse the JSON error envelope code from a response.
pub async fn err_code(resp: axum::http::Response<axum::body::Body>) -> String {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["error"]["code"].as_str().unwrap().to_string()
}
