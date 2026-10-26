mod config;
mod logging;
mod store;
mod web;

use std::sync::Arc;

use serde_json::json;
use tokio::net::TcpListener;

use crate::config::Config;
use crate::logging::JsonLogger;
use crate::store::{RedisStore, VisitCounter};
use crate::web::{AppState, router};

#[tokio::main]
async fn main() {
    std::process::exit(run().await);
}

async fn run() -> i32 {
    let logger = JsonLogger::stdout();

    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            logger.event(json!({"event": "startup_error", "error": error}));
            return 1;
        }
    };

    let store = match RedisStore::new(&config.redis_url) {
        Ok(store) => store,
        Err(error) => {
            logger.event(json!({
                "event": "redis_configuration_error",
                "error": error.to_string(),
            }));
            return 1;
        }
    };

    let args: Vec<String> = std::env::args().collect();

    if args.len() == 2 && args[1] == "reset" {
        return match store.reset().await {
            Ok(()) => {
                logger.event(json!({"event": "counter_reset", "status": "success"}));
                0
            }
            Err(error) => {
                logger.event(json!({
                    "event": "counter_reset_error",
                    "error": error.to_string(),
                }));
                1
            }
        };
    }

    if args.len() > 1 {
        logger.event(json!({
            "event": "usage_error",
            "error": "the only supported argument is reset",
        }));
        return 2;
    }

    let hostname = hostname::get()
        .ok()
        .and_then(|name| name.into_string().ok())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    let state = AppState {
        greeting: Arc::from(config.greeting),
        hostname: Arc::from(hostname),
        counter: Arc::new(store),
        logger: logger.clone(),
    };

    let app = router(state, logger.clone());

    let listener = match TcpListener::bind(("0.0.0.0", config.port)).await {
        Ok(listener) => listener,
        Err(error) => {
            logger.event(json!({
                "event": "server_error",
                "error": format!("bind port {}: {error}", config.port),
            }));
            return 1;
        }
    };

    logger.event(json!({"event": "server_started", "port": config.port}));

    if let Err(error) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(logger.clone()))
        .await
    {
        logger.event(json!({"event": "server_error", "error": error.to_string()}));
        return 1;
    }

    logger.event(json!({"event": "server_stopped", "status": "success"}));
    0
}

async fn shutdown_signal(logger: JsonLogger) {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = interrupt => {}
        _ = terminate => {}
    }

    logger.event(json!({"event": "shutdown_started"}));
}
