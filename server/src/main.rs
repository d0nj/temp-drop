use std::net::SocketAddr;
use std::time::Duration;
use tempdrop::api;
use tempdrop::config::Config;
use tempdrop::db::Db;
use tempdrop::janitor;
use tempdrop::rate_limit::RateLimiter;
use tempdrop::state::AppState;
use tempdrop::storage;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut config_path = None;
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg == "-c" || arg == "--config" {
            if let Some(val) = args.get(i + 1) {
                config_path = Some(std::path::PathBuf::from(val));
                i += 1;
            }
        } else if let Some(val) = arg.strip_prefix("-c=") {
            config_path = Some(std::path::PathBuf::from(val));
        } else if let Some(val) = arg.strip_prefix("--config=") {
            config_path = Some(std::path::PathBuf::from(val));
        }
        i += 1;
    }

    let config = Config::load(config_path.as_deref())?.validate()?;

    let db_path = if config.storage.backend == "local" {
        config.storage.root_dir.join("data.db")
    } else {
        config.storage.data_dir.join("data.db")
    };
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let db = Db::open(&db_path).await?;
    let storage = storage::build_storage(&config).await?;
    let limiter = RateLimiter::new(config.rate_limit.per_min);
    let state = AppState::new(db, storage, config.clone(), limiter);

    // startup sweep
    let now = chrono::Utc::now().timestamp();
    if let Ok(n) = janitor::sweep_once(&state, now).await {
        if n > 0 {
            println!("tempdrop: startup sweep removed {n} uploads");
        }
    }

    let interval = Duration::from_secs(config.janitor.interval_seconds);
    let janitor_state = state.clone();
    tokio::spawn(async move { janitor::run(janitor_state, interval).await });

    let app = api::router(state);
    let addr: SocketAddr = format!("{}:{}", config.server.bind, config.server.port).parse()?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!(
        "tempdrop: listening on http://{addr} (storage: {})",
        config.storage.backend
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("ctrl_c handler");
}
