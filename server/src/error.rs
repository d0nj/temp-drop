use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct ApiError {
    pub code: &'static str,
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    fn new(code: &'static str, status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            code,
            status,
            message: message.into(),
        }
    }
    pub fn bad_request(m: impl Into<String>) -> Self {
        Self::new("bad_request", StatusCode::BAD_REQUEST, m)
    }
    pub fn invalid_token() -> Self {
        Self::new("invalid_token", StatusCode::UNAUTHORIZED, "invalid upload token")
    }
    pub fn not_found() -> Self {
        Self::new("not_found", StatusCode::NOT_FOUND, "upload not found")
    }
    pub fn part_out_of_order() -> Self {
        Self::new("part_out_of_order", StatusCode::CONFLICT, "part number out of order")
    }
    pub fn already_complete() -> Self {
        Self::new("already_complete", StatusCode::CONFLICT, "upload is already complete")
    }
    pub fn not_complete() -> Self {
        Self::new("not_complete", StatusCode::CONFLICT, "upload has no parts yet")
    }
    pub fn part_too_large() -> Self {
        Self::new("part_too_large", StatusCode::PAYLOAD_TOO_LARGE, "part exceeds chunk size")
    }
    pub fn upload_too_large() -> Self {
        Self::new(
            "upload_too_large",
            StatusCode::PAYLOAD_TOO_LARGE,
            "file exceeds configured max upload size",
        )
    }
    pub fn no_expiry() -> Self {
        Self::new(
            "no_expiry",
            StatusCode::UNPROCESSABLE_ENTITY,
            "upload must have a ttl_seconds or max_downloads",
        )
    }
    pub fn rate_limited() -> Self {
        Self::new("rate_limited", StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded")
    }
    pub fn in_flight_limit() -> Self {
        Self::new(
            "in_flight_limit",
            StatusCode::TOO_MANY_REQUESTS,
            "too many bytes in flight, try again later",
        )
    }
    pub fn storage_low(free: u64) -> Self {
        Self::new(
            "storage_low",
            StatusCode::INSUFFICIENT_STORAGE,
            format!("storage low: {free} bytes free"),
        )
    }
    pub fn invalid_etag() -> Self {
        Self::new("invalid_etag", StatusCode::CONFLICT, "one or more part etags are invalid")
    }
    pub fn validation(m: impl Into<String>) -> Self {
        Self::new("validation", StatusCode::UNPROCESSABLE_ENTITY, m)
    }
    pub fn storage_error(m: impl Into<String>) -> Self {
        Self::new("storage_error", StatusCode::BAD_GATEWAY, m)
    }
    pub fn upstream(m: impl Into<String>) -> Self {
        Self::new("storage_error", StatusCode::SERVICE_UNAVAILABLE, m)
    }
    pub fn internal(m: impl Into<String>) -> Self {
        Self::new("internal", StatusCode::INTERNAL_SERVER_ERROR, m)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(json!({ "error": { "code": self.code, "message": self.message } }));
        (self.status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn error_envelope_shape() {
        let resp = ApiError::no_expiry().into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::UNPROCESSABLE_ENTITY);
        let body_bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let val: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(val["error"]["code"], "no_expiry");
    }
}
