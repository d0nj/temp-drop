pub mod download;
pub mod meta;
pub mod uploads;

use crate::state::AppState;
use axum::extract::State;
use axum::http::{StatusCode, Uri};
use axum::response::Response;
use axum::routing::{get, post, put};
use axum::Router;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../ui/dist"]
struct Assets;

const CSP_HEADER: &str = "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval' https:; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com data:; connect-src 'self' https: http: blob:; img-src 'self' data: blob: https:;";

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/api/uploads", post(uploads::start_upload))
        .route(
            "/api/uploads/{id}",
            get(meta::upload_meta).delete(uploads::abort_upload),
        )
        .route("/api/uploads/{id}/parts/{part}", put(uploads::upload_part))
        .route(
            "/api/uploads/{id}/parts/{part}/presign",
            get(uploads::presign_part),
        )
        .route(
            "/api/uploads/{id}/complete",
            post(uploads::complete_upload),
        )
        .route("/raw/{id}", get(download::raw_download))
        .fallback(get(static_or_index))
        .with_state(state)
}

/// Serve an embedded asset, falling back to index.html for SPA routes.
pub async fn static_or_index(
    State(_st): State<AppState>,
    uri: Uri,
) -> Result<Response, (StatusCode, String)> {
    let path = uri.path().trim_start_matches('/');

    if let Ok(dev_dir) = std::env::var("FILEHOST_UI_DIR") {
        let dev_path = std::path::PathBuf::from(dev_dir);
        let target_file = if path.is_empty() || path == "index.html" {
            dev_path.join("index.html")
        } else {
            let requested = dev_path.join(path);
            if requested.exists() {
                requested
            } else if !path.starts_with("api/") {
                dev_path.join("index.html")
            } else {
                return Err((StatusCode::NOT_FOUND, "not found".into()));
            }
        };

        let data = tokio::fs::read(&target_file)
            .await
            .map_err(|_| (StatusCode::NOT_FOUND, "failed to read dev asset".into()))?;
        let mime = mime_guess::from_path(&target_file)
            .first_or_octet_stream()
            .to_string();

        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", mime)
            .header("Content-Security-Policy", CSP_HEADER)
            .header("X-Content-Type-Options", "nosniff")
            .header("X-Frame-Options", "DENY")
            .header("Referrer-Policy", "no-referrer")
            .body(axum::body::Body::from(data))
            .unwrap());
    }

    let (data, mime) = if path.is_empty() || path == "index.html" {
        (
            Assets::get("index.html")
                .ok_or((StatusCode::NOT_FOUND, "missing index.html".into()))?
                .data,
            "text/html".to_string(),
        )
    } else if let Some(f) = Assets::get(path) {
        (
            f.data,
            mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string(),
        )
    } else if !path.starts_with("api/") {
        // SPA history route (e.g. /f/abc123) — serve the app shell
        (
            Assets::get("index.html")
                .ok_or((StatusCode::NOT_FOUND, "missing index.html".into()))?
                .data,
            "text/html".to_string(),
        )
    } else {
        return Err((StatusCode::NOT_FOUND, "not found".into()));
    };
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", mime)
        .header("Content-Security-Policy", CSP_HEADER)
        .header("X-Content-Type-Options", "nosniff")
        .header("X-Frame-Options", "DENY")
        .header("Referrer-Policy", "no-referrer")
        .body(axum::body::Body::from(data))
        .unwrap())
}
