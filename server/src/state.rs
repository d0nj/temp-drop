use crate::config::Config;
use crate::db::Db;
use crate::rate_limit::RateLimiter;
use crate::storage::Storage;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub storage: Arc<Storage>,
    pub config: Arc<Config>,
    pub limiter: Arc<RateLimiter>,
}

impl AppState {
    pub fn new(db: Db, storage: Storage, config: Config, limiter: RateLimiter) -> Self {
        Self {
            db: Arc::new(db),
            storage: Arc::new(storage),
            config: Arc::new(config),
            limiter: Arc::new(limiter),
        }
    }
}
