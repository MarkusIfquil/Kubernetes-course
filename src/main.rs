use std::env;

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    std::process::exit(run().await);
}

async fn run() -> i32 {
    let port_text = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let Some(port) = port_text.parse::<u16>().ok().filter(|port| *port >= 1) else {
        eprintln!("invalid PORT {port_text:?}: must be an integer between 1 and 65535");
        return 1;
    };

    let listener = match TcpListener::bind(("0.0.0.0", port)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("bind port {port}: {error}");
            return 1;
        }
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/healthz", get(healthz));

    eprintln!("listening on :{port}");

    if let Err(error) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        eprintln!("server error: {error}");
        return 1;
    }

    0
}

async fn root() -> Response {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "hello from version 0.1.1.\n",
    )
        .into_response()
}

async fn healthz() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "ok\n",
    )
        .into_response()
}

async fn shutdown_signal() {
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
}
