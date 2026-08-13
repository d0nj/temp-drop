use crate::error::ApiError;
use crate::state::AppState;
use crate::storage::{Storage, StorageError};
use axum::body::Body;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Deserialize)]
pub struct StartReq {
    pub name: String,
    #[serde(default)]
    pub size_bytes: Option<i64>,
    #[serde(default)]
    pub ttl_seconds: Option<i64>,
    #[serde(default)]
    pub max_downloads: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct StartRes {
    pub id: String,
    pub upload_token: String,
    pub chunk_size: u64,
    pub backend: String,
    pub expires_at: Option<i64>,
    pub max_downloads: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PartPath {
    pub id: String,
    pub part: i64,
}

#[derive(Debug, Serialize)]
pub struct PartRes {
    pub received: i64,
}

#[derive(Debug, Serialize)]
pub struct PresignRes {
    pub url: String,
    pub part: i64,
}

#[derive(Debug, Deserialize)]
pub struct IdPath {
    pub id: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct CompleteReq {
    #[serde(default)]
    pub etags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CompleteRes {
    pub url: String,
}

fn token_of(headers: &HeaderMap) -> Option<&str> {
    headers.get("x-upload-token").and_then(|v| v.to_str().ok())
}

async fn read_limited(body: Body, limit: usize) -> Result<Vec<u8>, ApiError> {
    let mut out = Vec::new();
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ApiError::internal(e.to_string()))?;
        if out.len() + chunk.len() > limit {
            return Err(ApiError::part_too_large());
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

fn client_ip(st: &AppState, headers: &HeaderMap, connect: ConnectInfo<SocketAddr>) -> IpAddr {
    if st.config.server.trust_proxy {
        // Priority 1: Cloudflare CF-Connecting-IP header
        if let Some(cf_ip) = headers
            .get("cf-connecting-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse().ok())
        {
            return cf_ip;
        }

        // Priority 2: X-Forwarded-For header (first IP in chain)
        if let Some(xff_ip) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .and_then(|v| v.trim().parse().ok())
        {
            return xff_ip;
        }

        connect.0.ip()
    } else {
        connect.0.ip()
    }
}

pub async fn start_upload(
    State(st): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<StartReq>,
) -> Result<(StatusCode, Json<StartRes>), ApiError> {
    let now = chrono::Utc::now().timestamp();
    let ip = client_ip(&st, &headers, ConnectInfo(addr));
    if !st.limiter.check(ip, now) {
        return Err(ApiError::rate_limited());
    }

    let name = crate::id::sanitize_name(&req.name).map_err(|e| ApiError::validation(e.to_string()))?;
    let ttl = req.ttl_seconds.filter(|t| *t > 0).map(|t| t.min(st.config.uploads.max_ttl_seconds));
    let max_downloads = req.max_downloads.filter(|m| *m > 0).map(|m| m.min(st.config.uploads.max_downloads));
    if ttl.is_none() && max_downloads.is_none() {
        return Err(ApiError::no_expiry());
    }
    if let (Some(cap), Some(declared)) = (st.config.max_upload_size(), req.size_bytes) {
        if declared > cap {
            return Err(ApiError::upload_too_large());
        }
    }

    let inflight = st.db.pending_local_bytes().await.map_err(|e| ApiError::internal(e.to_string()))?;
    if inflight >= st.config.uploads.max_in_flight_bytes {
        return Err(ApiError::in_flight_limit());
    }

    let id = crate::id::new_id();
    let token = crate::id::new_token();
    let expires_at = ttl.map(|t| now + t);

    let row = crate::db::UploadRow {
        id: id.clone(),
        name,
        size: 0,
        backend: st.storage.kind().to_string(),
        storage_key: id.clone(),
        status: "pending".into(),
        received_bytes: 0,
        part_count: 0,
        s3_upload_id: None,
        ttl_seconds: ttl,
        max_downloads,
        download_count: 0,
        upload_token: token.clone(),
        created_at: now,
        expires_at,
    };
    st.db.create_upload(&row).await.map_err(|e| ApiError::internal(e.to_string()))?;

    match &*st.storage {
        Storage::Local(s) => {
            s.check_free().map_err(|e| match e {
                StorageError::LowSpace(free) => ApiError::storage_low(free),
                other => ApiError::storage_error(other.to_string()),
            })?;
            s.create(&id).await.map_err(|e| ApiError::storage_error(e.to_string()))?;
        }
        Storage::S3(s) => {
            let upload_id = s.create_multipart(&id).await.map_err(crate::storage::map_s3_error)?;
            st.db.set_s3_upload_id(&id, &upload_id).await.map_err(|e| ApiError::internal(e.to_string()))?;
        }
    }

    Ok((
        StatusCode::CREATED,
        Json(StartRes {
            id,
            upload_token: token,
            chunk_size: st.config.chunk_size() as u64,
            backend: st.storage.kind().to_string(),
            expires_at,
            max_downloads,
        }),
    ))
}

pub async fn upload_part(
    State(st): State<AppState>,
    Path(path): Path<PartPath>,
    headers: HeaderMap,
    body: Body,
) -> Result<Json<PartRes>, ApiError> {
    let row = st
        .db
        .get_upload(&path.id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(ApiError::not_found)?;

    if row.status != "pending" {
        return Err(ApiError::already_complete());
    }
    let token = token_of(&headers).ok_or_else(ApiError::invalid_token)?;
    if token != row.upload_token {
        return Err(ApiError::invalid_token());
    }

    let bytes = read_limited(body, st.config.chunk_size()).await?;

    let Storage::Local(local) = &*st.storage else {
        return Err(ApiError::bad_request(
            "part uploads go to the presign endpoint for s3",
        ));
    };

    local.check_free().map_err(|e| match e {
        StorageError::LowSpace(free) => ApiError::storage_low(free),
        other => ApiError::storage_error(other.to_string()),
    })?;

    let ok = st
        .db
        .take_part(&row.id, path.part, bytes.len() as i64)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if !ok {
        return Err(ApiError::part_out_of_order());
    }

    local
        .append(&row.storage_key, &bytes)
        .await
        .map_err(|e| ApiError::storage_error(e.to_string()))?;

    Ok(Json(PartRes {
        received: row.received_bytes + bytes.len() as i64,
    }))
}

pub async fn presign_part(
    State(st): State<AppState>,
    Path(path): Path<PartPath>,
    headers: HeaderMap,
) -> Result<Json<PresignRes>, ApiError> {
    let row = st
        .db
        .get_upload(&path.id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(ApiError::not_found)?;
    if row.status != "pending" {
        return Err(ApiError::already_complete());
    }
    let token = token_of(&headers).ok_or_else(ApiError::invalid_token)?;
    if token != row.upload_token {
        return Err(ApiError::invalid_token());
    }
    let Storage::S3(s3) = &*st.storage else {
        return Err(ApiError::bad_request(
            "presign is only available with the s3 backend",
        ));
    };
    let ok = st
        .db
        .bump_part_count(&row.id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if !ok {
        return Err(ApiError::already_complete());
    }
    let part = row.part_count + 1;
    let upload_id = row
        .s3_upload_id
        .as_deref()
        .ok_or_else(|| ApiError::internal("missing s3 upload id"))?;
    let url = s3
        .presign_part(&row.storage_key, upload_id, part as i32)
        .await
        .map_err(crate::storage::map_s3_error)?;
    Ok(Json(PresignRes { url, part }))
}

pub async fn complete_upload(
    State(st): State<AppState>,
    Path(path): Path<IdPath>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<CompleteRes>, ApiError> {
    let row = st
        .db
        .get_upload(&path.id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(ApiError::not_found)?;

    if row.status != "pending" {
        return Err(ApiError::already_complete());
    }
    let token = token_of(&headers).ok_or_else(ApiError::invalid_token)?;
    if token != row.upload_token {
        return Err(ApiError::invalid_token());
    }

    match &*st.storage {
        Storage::Local(local) => {
            if row.received_bytes == 0 {
                return Err(ApiError::not_complete());
            }
            if let Some(cap) = st.config.max_upload_size() {
                if row.received_bytes > cap {
                    let _ = local.delete(&row.storage_key).await;
                    let _ = st.db.delete_upload(&row.id).await;
                    return Err(ApiError::upload_too_large());
                }
            }
            local
                .complete(&row.storage_key)
                .await
                .map_err(|e| ApiError::storage_error(e.to_string()))?;
            st.db
                .complete_upload(&row.id, row.received_bytes)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            Ok(Json(CompleteRes {
                url: format!("/f/{}", row.id),
            }))
        }
        Storage::S3(s3) => {
            let req: CompleteReq = if body.is_empty() {
                CompleteReq::default()
            } else {
                serde_json::from_slice(&body).map_err(|e| ApiError::bad_request(e.to_string()))?
            };
            if req.etags.len() as i64 != row.part_count {
                return Err(ApiError::not_complete());
            }
            let upload_id = row
                .s3_upload_id
                .as_deref()
                .ok_or_else(|| ApiError::internal("missing s3 upload id"))?;
            let size = s3
                .complete(&row.storage_key, upload_id, &req.etags)
                .await
                .map_err(crate::storage::map_s3_error)? as i64;
            if let Some(cap) = st.config.max_upload_size() {
                if size > cap {
                    let _ = s3.abort(&row.storage_key, upload_id).await;
                    let _ = st.db.delete_upload(&row.id).await;
                    return Err(ApiError::upload_too_large());
                }
            }
            st.db
                .complete_upload(&row.id, size)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            Ok(Json(CompleteRes {
                url: format!("/f/{}", row.id),
            }))
        }
    }
}

pub async fn abort_upload(
    State(st): State<AppState>,
    Path(path): Path<IdPath>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let row = st
        .db
        .get_upload(&path.id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(ApiError::not_found)?;

    if row.status != "pending" {
        return Err(ApiError::already_complete());
    }
    let token = token_of(&headers).ok_or_else(ApiError::invalid_token)?;
    if token != row.upload_token {
        return Err(ApiError::invalid_token());
    }

    match &*st.storage {
        Storage::Local(local) => {
            local
                .delete(&row.storage_key)
                .await
                .map_err(|e| ApiError::storage_error(e.to_string()))?;
        }
        Storage::S3(s3) => {
            if let Some(uid) = row.s3_upload_id.as_deref() {
                s3.abort(&row.storage_key, uid)
                    .await
                    .map_err(crate::storage::map_s3_error)?;
            }
        }
    }
    st.db
        .delete_upload(&row.id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
