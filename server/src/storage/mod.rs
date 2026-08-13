pub mod local;
pub mod s3;

use bytes::Bytes;
use futures_util::stream::BoxStream;

pub type ByteStream = BoxStream<'static, Result<Bytes, std::io::Error>>;

pub struct OpenResult {
    pub total: u64, // full object size
    pub start: u64, // first byte served
    pub end: u64,   // last byte served (inclusive)
    pub stream: ByteStream,
}

impl std::fmt::Debug for OpenResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenResult")
            .field("total", &self.total)
            .field("start", &self.start)
            .field("end", &self.end)
            .finish()
    }
}

#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    LowSpace(u64),
    S3(String),
    NoSuchKey,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "storage io: {e}"),
            Self::LowSpace(free) => write!(f, "storage low: {free} bytes free"),
            Self::S3(m) => write!(f, "s3 error: {m}"),
            Self::NoSuchKey => write!(f, "key not found"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub fn map_s3_error(e: StorageError) -> crate::error::ApiError {
    match &e {
        StorageError::S3(msg) if msg.contains("NoSuchKey") => crate::error::ApiError::not_found(),
        StorageError::S3(msg)
            if msg.contains("InvalidPart")
                || msg.contains("invalid_etag")
                || msg.contains("Invalid ETag") =>
        {
            crate::error::ApiError::invalid_etag()
        }
        StorageError::S3(_) => crate::error::ApiError::upstream(e.to_string()),
        StorageError::NoSuchKey => crate::error::ApiError::not_found(),
        StorageError::Io(e) => crate::error::ApiError::storage_error(e.to_string()),
        StorageError::LowSpace(f) => crate::error::ApiError::storage_low(*f),
    }
}

pub enum Storage {
    Local(local::LocalStorage),
    S3(s3::S3Storage),
}

impl Storage {
    pub fn kind(&self) -> &'static str {
        match self {
            Storage::Local(_) => "local",
            Storage::S3(_) => "s3",
        }
    }

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        match self {
            Storage::Local(s) => s.delete(key).await,
            Storage::S3(s) => s.delete(key).await,
        }
    }

    pub async fn open_range(
        &self,
        key: &str,
        range: Option<(u64, u64)>,
    ) -> Result<OpenResult, StorageError> {
        match self {
            Storage::Local(s) => s.open_range(key, range).await,
            Storage::S3(s) => s.open_range(key, range).await,
        }
    }
}

pub async fn build_storage(config: &crate::config::Config) -> Result<Storage, StorageError> {
    match config.storage.backend.as_str() {
        "local" => Ok(Storage::Local(local::LocalStorage::new(
            config.storage.root_dir.clone(),
            config.uploads.min_free_bytes,
        )?)),
        "s3" => Ok(Storage::S3(s3::S3Storage::new(&config.storage.s3).await?)),
        other => Err(StorageError::S3(format!("unknown backend {other}"))),
    }
}
