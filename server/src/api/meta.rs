use crate::api::uploads::IdPath;
use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct MetaRes {
    pub id: String,
    pub name: String,
    pub size: i64,
    pub backend: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub downloads_left: Option<i64>,
    pub max_downloads: Option<i64>,
    pub status: String,
}

pub async fn upload_meta(
    State(st): State<AppState>,
    Path(path): Path<IdPath>,
) -> Result<Json<MetaRes>, ApiError> {
    let row = st
        .db
        .get_upload(&path.id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(ApiError::not_found)?;
    let now = chrono::Utc::now().timestamp();
    if row.status != "ready" {
        return Err(ApiError::not_found());
    }
    if let Some(exp) = row.expires_at {
        if exp <= now {
            return Err(ApiError::not_found());
        }
    }
    if let Some(max) = row.max_downloads {
        if row.download_count >= max {
            return Err(ApiError::not_found());
        }
    }
    let downloads_left = row.max_downloads.map(|m| (m - row.download_count).max(0));
    Ok(Json(MetaRes {
        id: row.id.clone(),
        name: row.name,
        size: row.size,
        backend: row.backend,
        created_at: row.created_at,
        expires_at: row.expires_at,
        downloads_left,
        max_downloads: row.max_downloads,
        status: row.status,
    }))
}
