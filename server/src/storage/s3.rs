use super::{ByteStream, OpenResult, StorageError};
use crate::config::S3Config;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region, SharedCredentialsProvider};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use aws_sdk_s3::Client;
use std::time::Duration;
use tokio_util::io::ReaderStream;

pub struct S3Storage {
    client: Client,
    bucket: String,
    presign_ttl: Duration,
}

impl S3Storage {
    pub async fn new(cfg: &S3Config) -> Result<Self, StorageError> {
        if cfg.bucket.is_empty() {
            return Err(StorageError::S3("s3 bucket is empty".into()));
        }
        let creds = Credentials::new(
            cfg.access_key.clone(),
            cfg.secret_key.clone(),
            None,
            None,
            "tempdrop",
        );
        let mut builder = aws_config::SdkConfig::builder()
            .region(Region::new(cfg.region.clone()))
            .credentials_provider(SharedCredentialsProvider::new(creds))
            .behavior_version(BehaviorVersion::latest());
        if !cfg.endpoint.is_empty() {
            builder = builder.endpoint_url(cfg.endpoint.clone());
        }
        let sdk_config = builder.build();
        let s3_config = aws_sdk_s3::config::Builder::from(&sdk_config)
            .force_path_style(cfg.force_path_style)
            .build();
        let client = Client::from_conf(s3_config);
        Ok(Self {
            client,
            bucket: cfg.bucket.clone(),
            presign_ttl: Duration::from_secs(cfg.presign_ttl_seconds),
        })
    }

    pub async fn create_multipart(&self, key: &str) -> Result<String, StorageError> {
        let resp = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StorageError::S3(e.to_string()))?;
        resp.upload_id
            .ok_or_else(|| StorageError::S3("create_multipart_upload returned no upload_id".into()))
    }

    pub async fn presign_part(
        &self,
        key: &str,
        upload_id: &str,
        part: i32,
    ) -> Result<String, StorageError> {
        let cfg = PresigningConfig::expires_in(self.presign_ttl)
            .map_err(|e| StorageError::S3(format!("presign ttl: {e}")))?;
        let uri = self
            .client
            .upload_part()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(upload_id)
            .part_number(part)
            .presigned(cfg)
            .await
            .map_err(|e| StorageError::S3(e.to_string()))?;
        Ok(uri.uri().to_string())
    }

    pub async fn complete(
        &self,
        key: &str,
        upload_id: &str,
        etags: &[String],
    ) -> Result<u64, StorageError> {
        let parts: Vec<CompletedPart> = etags
            .iter()
            .enumerate()
            .map(|(i, etag)| {
                CompletedPart::builder()
                    .part_number(i as i32 + 1)
                    .e_tag(etag)
                    .build()
            })
            .collect();
        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(upload_id)
            .multipart_upload(
                CompletedMultipartUpload::builder()
                    .set_parts(Some(parts))
                    .build(),
            )
            .send()
            .await
            .map_err(|e| StorageError::S3(e.to_string()))?;
        let head = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StorageError::S3(e.to_string()))?;
        head.content_length()
            .map(|l| l as u64)
            .ok_or_else(|| StorageError::S3("head_object returned no content_length".into()))
    }

    pub async fn abort(&self, key: &str, upload_id: &str) -> Result<(), StorageError> {
        self.client
            .abort_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(upload_id)
            .send()
            .await
            .map_err(|e| StorageError::S3(e.to_string()))?;
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StorageError::S3(e.to_string()))?;
        Ok(())
    }

    pub async fn open_range(
        &self,
        key: &str,
        range: Option<(u64, u64)>,
    ) -> Result<OpenResult, StorageError> {
        let mut req = self.client.get_object().bucket(&self.bucket).key(key);
        if let Some((start, end)) = range {
            req = req.range(format!("bytes={start}-{end}"));
        }
        let resp = req
            .send()
            .await
            .map_err(|e| StorageError::S3(e.to_string()))?;
        let total = resp.content_length().map(|l| l as u64).unwrap_or(0);
        let (start, end) = match (range, resp.content_range()) {
            (Some(_), Some(cr)) => {
                let parsed = parse_content_range(cr)
                    .ok_or_else(|| StorageError::S3("bad content-range".into()))?;
                (parsed.0, parsed.1)
            }
            _ => (0, total.saturating_sub(1)),
        };
        if total == 0 {
            return Err(StorageError::NoSuchKey);
        }
        let stream: ByteStream = Box::pin(ReaderStream::new(resp.body.into_async_read()));
        Ok(OpenResult {
            total,
            start,
            end,
            stream,
        })
    }
}

/// Parse "bytes a-b/total" → (a, b).
fn parse_content_range(value: &str) -> Option<(u64, u64)> {
    let rest = value.trim().strip_prefix("bytes ")?;
    let (range, _total) = rest.split_once('/')?;
    let (a, b) = range.split_once('-')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::map_s3_error;

    #[test]
    fn map_no_such_key_to_not_found() {
        let e = map_s3_error(StorageError::S3("service error: NoSuchKey".into()));
        assert_eq!(e.status, axum::http::StatusCode::NOT_FOUND);
    }

    #[test]
    fn map_unknown_to_upstream() {
        let e = map_s3_error(StorageError::S3("network glitch".into()));
        assert_eq!(e.status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn new_fails_on_empty_bucket() {
        let cfg = S3Config::default(); // bucket is empty
        assert!(S3Storage::new(&cfg).await.is_err());
    }
}
