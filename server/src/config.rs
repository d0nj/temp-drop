use serde::Deserialize;
use std::env;
use std::path::{Path, PathBuf};

const S3_MIN_PART: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub uploads: UploadsConfig,
    pub rate_limit: RateLimitConfig,
    pub janitor: JanitorConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            storage: StorageConfig::default(),
            uploads: UploadsConfig::default(),
            rate_limit: RateLimitConfig::default(),
            janitor: JanitorConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    pub port: u16,
    pub bind: String,
    pub trust_proxy: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            bind: "0.0.0.0".into(),
            trust_proxy: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StorageConfig {
    pub backend: String,
    pub root_dir: PathBuf,
    pub data_dir: PathBuf,
    pub s3: S3Config,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: "local".into(),
            root_dir: PathBuf::from("./data"),
            data_dir: PathBuf::from("./data"),
            s3: S3Config::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct S3Config {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub force_path_style: bool,
    pub presign_ttl_seconds: u64,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            bucket: String::new(),
            region: "auto".into(),
            access_key: String::new(),
            secret_key: String::new(),
            force_path_style: false,
            presign_ttl_seconds: 900,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UploadsConfig {
    pub chunk_size_bytes: u64,
    pub max_upload_size_bytes: u64,
    pub max_ttl_seconds: i64,
    pub max_downloads: i64,
    pub max_in_flight_bytes: i64,
    pub min_free_bytes: u64,
    pub pending_timeout_hours: i64,
}

impl Default for UploadsConfig {
    fn default() -> Self {
        Self {
            chunk_size_bytes: 33_554_432,
            max_upload_size_bytes: 0,
            max_ttl_seconds: 2_592_000,
            max_downloads: 10_000,
            max_in_flight_bytes: 8_589_934_592,
            min_free_bytes: 1_073_741_824,
            pending_timeout_hours: 24,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RateLimitConfig {
    pub per_min: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self { per_min: 120 }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JanitorConfig {
    pub interval_seconds: u64,
}

impl Default for JanitorConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 60,
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(toml::de::Error),
    UnknownKey(String),
    Invalid(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "config io: {e}"),
            Self::Parse(e) => write!(f, "config parse: {e}"),
            Self::UnknownKey(k) => write!(f, "unknown config key: {k}"),
            Self::Invalid(m) => write!(f, "invalid config: {m}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        let msg = e.to_string();
        if msg.contains("unknown field") {
            Self::UnknownKey(msg)
        } else {
            Self::Parse(e)
        }
    }
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Config, ConfigError> {
        let mut config = match path {
            Some(p) => {
                let text = std::fs::read_to_string(p)?;
                toml::from_str(&text)?
            }
            None => {
                let default_path = Path::new("config.toml");
                if default_path.exists() {
                    let text = std::fs::read_to_string(default_path)?;
                    toml::from_str(&text)?
                } else {
                    Config::default()
                }
            }
        };
        config.apply_env();
        Ok(config)
    }

    fn apply_env(&mut self) {
        if let Ok(v) = env::var("FILEHOST_SERVER_PORT") {
            self.server.port = v.parse().expect("FILEHOST_SERVER_PORT must be u16");
        }
        if let Ok(v) = env::var("FILEHOST_SERVER_BIND") {
            self.server.bind = v;
        }
        if let Ok(v) = env::var("FILEHOST_SERVER_TRUST_PROXY") {
            self.server.trust_proxy = v == "true" || v == "1";
        }
        if let Ok(v) = env::var("FILEHOST_STORAGE_BACKEND") {
            self.storage.backend = v;
        }
        if let Ok(v) = env::var("FILEHOST_STORAGE_ROOT_DIR") {
            self.storage.root_dir = v.into();
        }
        if let Ok(v) = env::var("FILEHOST_STORAGE_DATA_DIR") {
            self.storage.data_dir = v.into();
        }
        if let Ok(v) = env::var("FILEHOST_STORAGE_S3_ENDPOINT") {
            self.storage.s3.endpoint = v;
        }
        if let Ok(v) = env::var("FILEHOST_STORAGE_S3_BUCKET") {
            self.storage.s3.bucket = v;
        }
        if let Ok(v) = env::var("FILEHOST_STORAGE_S3_REGION") {
            self.storage.s3.region = v;
        }
        if let Ok(v) = env::var("FILEHOST_STORAGE_S3_ACCESS_KEY") {
            self.storage.s3.access_key = v;
        }
        if let Ok(v) = env::var("FILEHOST_STORAGE_S3_SECRET_KEY") {
            self.storage.s3.secret_key = v;
        }
        if let Ok(v) = env::var("FILEHOST_STORAGE_S3_FORCE_PATH_STYLE") {
            self.storage.s3.force_path_style = v == "true" || v == "1";
        }
        if let Ok(v) = env::var("FILEHOST_STORAGE_S3_PRESIGN_TTL_SECONDS") {
            self.storage.s3.presign_ttl_seconds = v.parse().unwrap();
        }
        if let Ok(v) = env::var("FILEHOST_UPLOADS_CHUNK_SIZE_BYTES") {
            self.uploads.chunk_size_bytes = v.parse().unwrap();
        }
        if let Ok(v) = env::var("FILEHOST_UPLOADS_MAX_UPLOAD_SIZE_BYTES") {
            self.uploads.max_upload_size_bytes = v.parse().unwrap();
        }
        if let Ok(v) = env::var("FILEHOST_UPLOADS_MAX_TTL_SECONDS") {
            self.uploads.max_ttl_seconds = v.parse().unwrap();
        }
        if let Ok(v) = env::var("FILEHOST_UPLOADS_MAX_DOWNLOADS") {
            self.uploads.max_downloads = v.parse().unwrap();
        }
        if let Ok(v) = env::var("FILEHOST_UPLOADS_MAX_IN_FLIGHT_BYTES") {
            self.uploads.max_in_flight_bytes = v.parse().unwrap();
        }
        if let Ok(v) = env::var("FILEHOST_UPLOADS_MIN_FREE_BYTES") {
            self.uploads.min_free_bytes = v.parse().unwrap();
        }
        if let Ok(v) = env::var("FILEHOST_UPLOADS_PENDING_TIMEOUT_HOURS") {
            self.uploads.pending_timeout_hours = v.parse().unwrap();
        }
        if let Ok(v) = env::var("FILEHOST_RATE_LIMIT_PER_MIN") {
            self.rate_limit.per_min = v.parse().unwrap();
        }
        if let Ok(v) = env::var("FILEHOST_JANITOR_INTERVAL_SECONDS") {
            self.janitor.interval_seconds = v.parse().unwrap();
        }
    }

    pub fn validate(self) -> Result<Config, ConfigError> {
        match self.storage.backend.as_str() {
            "local" => {}
            "s3" => {
                let s3 = &self.storage.s3;
                if s3.bucket.is_empty() || s3.access_key.is_empty() || s3.secret_key.is_empty() {
                    return Err(ConfigError::Invalid(
                        "storage.backend=s3 requires bucket, access_key, secret_key".to_string(),
                    ));
                }
            }
            other => {
                return Err(ConfigError::Invalid(format!(
                    "storage.backend must be 'local' or 's3', got '{other}'"
                )));
            }
        }
        if self.uploads.chunk_size_bytes < S3_MIN_PART {
            return Err(ConfigError::Invalid(format!(
                "uploads.chunk_size_bytes must be >= {S3_MIN_PART} (S3 part floor)"
            )));
        }
        if self.uploads.max_ttl_seconds <= 0 {
            return Err(ConfigError::Invalid(
                "uploads.max_ttl_seconds must be > 0".to_string(),
            ));
        }
        if self.uploads.max_downloads <= 0 {
            return Err(ConfigError::Invalid(
                "uploads.max_downloads must be > 0".to_string(),
            ));
        }
        if self.rate_limit.per_min == 0 {
            return Err(ConfigError::Invalid(
                "rate_limit.per_min must be > 0".to_string(),
            ));
        }
        Ok(self)
    }

    pub fn chunk_size(&self) -> usize {
        self.uploads.chunk_size_bytes as usize
    }

    pub fn max_upload_size(&self) -> Option<i64> {
        if self.uploads.max_upload_size_bytes > 0 {
            Some(self.uploads.max_upload_size_bytes as i64)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let c = Config::default();
        assert_eq!(c.server.port, 8080);
        assert_eq!(c.storage.backend, "local");
        assert_eq!(c.uploads.chunk_size_bytes, 33_554_432);
        assert_eq!(c.uploads.max_upload_size_bytes, 0);
        assert_eq!(c.uploads.max_ttl_seconds, 2_592_000);
        assert_eq!(c.rate_limit.per_min, 120);
        assert_eq!(c.janitor.interval_seconds, 60);
    }

    #[test]
    fn toml_loads_and_env_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[server]\nport = 9000\n[storage.s3]\nbucket = \"bkt\"\n",
        )
        .unwrap();
        unsafe {
            std::env::set_var("FILEHOST_RATE_LIMIT_PER_MIN", "42");
            std::env::set_var("FILEHOST_SERVER_PORT", "9001");
        }
        let c = Config::load(Some(&path)).unwrap();
        unsafe {
            std::env::remove_var("FILEHOST_RATE_LIMIT_PER_MIN");
            std::env::remove_var("FILEHOST_SERVER_PORT");
        }
        assert_eq!(c.server.port, 9001); // env beats file
        assert_eq!(c.storage.s3.bucket, "bkt"); // file applies
        assert_eq!(c.rate_limit.per_min, 42); // env applies
    }

    #[test]
    fn unknown_key_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[server]\nportt = 1\n").unwrap();
        let err = Config::load(Some(&path)).unwrap_err();
        assert!(matches!(err, ConfigError::UnknownKey(_)));
    }

    #[test]
    fn validation_rejects_bad_values() {
        let mut c = Config::default();
        c.storage.backend = "tape".into();
        assert!(c.clone().validate().is_err());
        c.storage.backend = "s3".into();
        assert!(c.clone().validate().is_err()); // s3 needs keys
        let mut c2 = Config::default();
        c2.uploads.chunk_size_bytes = 1_000_000; // below S3 floor
        assert!(c2.validate().is_err());
    }
}
