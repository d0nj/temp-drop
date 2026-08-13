use crate::error::ApiError;
use crate::id::{header_filename, pct_encode};
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;

#[derive(Debug, PartialEq, Eq)]
pub enum RangeSpec {
    Bytes(u64, u64),
    Suffix(u64),
}

pub fn parse_range(h: Option<&str>) -> Option<RangeSpec> {
    let h = h?;
    let h = h.trim();
    let rest = h.strip_prefix("bytes=")?;
    if rest.contains(',') {
        return None;
    } // multi-range: serve full
    if let Some(n) = rest.strip_prefix('-') {
        let n: u64 = n.parse().ok()?;
        return (n > 0).then_some(RangeSpec::Suffix(n));
    }
    let (a, b) = rest.split_once('-')?;
    let start: u64 = a.parse().ok()?;
    let end = if b.is_empty() {
        u64::MAX
    } else {
        b.parse().ok()?
    };
    if start > end && end != u64::MAX {
        return None;
    }
    Some(RangeSpec::Bytes(start, end))
}

fn content_disposition(name: &str) -> String {
    format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        header_filename(name),
        pct_encode(name)
    )
}

pub async fn raw_download(
    State(st): State<AppState>,
    Path(path): Path<crate::api::uploads::IdPath>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let now = chrono::Utc::now().timestamp();
    let allowed = st
        .db
        .increment_download(&path.id, now)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if !allowed {
        return Err(ApiError::not_found());
    }
    let row = st
        .db
        .get_upload(&path.id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(ApiError::not_found)?;

    let spec = parse_range(headers.get(header::RANGE).and_then(|v| v.to_str().ok()));
    // Always open full first to learn total, then re-open the resolved window.
    let open = st
        .storage
        .open_range(&row.storage_key, None)
        .await
        .map_err(storage_to_api)?;
    let (start, end, ranged) = match spec {
        None => (open.start, open.end, false),
        Some(RangeSpec::Bytes(a, b)) => {
            let b = b.min(open.total.saturating_sub(1));
            if a > b {
                (
                    open.total.saturating_sub(1),
                    open.total.saturating_sub(1),
                    true,
                )
            } else {
                (a, b, true)
            }
        }
        Some(RangeSpec::Suffix(n)) => {
            let start = open.total.saturating_sub(n);
            (start, open.total.saturating_sub(1), true)
        }
    };
    let open = if ranged {
        st.storage
            .open_range(&row.storage_key, Some((start, end)))
            .await
            .map_err(storage_to_api)?
    } else {
        open
    };

    let len = end - start + 1;
    let mut builder = Response::builder()
        .status(if ranged {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, content_disposition(&row.name))
        .header("Accept-Ranges", "bytes")
        .header(header::CACHE_CONTROL, "private, no-store")
        .header("X-Content-Type-Options", "nosniff")
        .header("Referrer-Policy", "no-referrer")
        .header(header::CONTENT_LENGTH, len);
    if ranged {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", start, end, open.total),
        );
    }
    Ok(builder.body(Body::from_stream(open.stream)).unwrap())
}

fn storage_to_api(e: crate::storage::StorageError) -> ApiError {
    match e {
        crate::storage::StorageError::NoSuchKey => ApiError::not_found(),
        crate::storage::StorageError::Io(e) => ApiError::storage_error(e.to_string()),
        crate::storage::StorageError::LowSpace(f) => ApiError::storage_low(f),
        crate::storage::StorageError::S3(m) => ApiError::upstream(m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_range_cases() {
        assert!(matches!(parse_range(Some("bytes=0-499")), Some(RangeSpec::Bytes(0, 499))));
        assert!(matches!(parse_range(Some("bytes=500-")), Some(RangeSpec::Bytes(500, u64::MAX))));
        assert!(matches!(parse_range(Some("bytes=-500")), Some(RangeSpec::Suffix(500))));
        assert!(parse_range(Some("bytes=0-10,20-30")).is_none());
        assert!(parse_range(Some("invalid")).is_none());
    }
}
