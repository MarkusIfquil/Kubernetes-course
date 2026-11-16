use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::State;
use axum::http::{Request, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::json;

use crate::logging::JsonLogger;
use crate::store::VisitCounter;

#[derive(Clone)]
pub struct AppState {
    pub greeting: Arc<str>,
    pub hostname: Arc<str>,
    pub counter: Arc<dyn VisitCounter>,
    pub logger: JsonLogger,
}

pub fn router(state: AppState, logger: JsonLogger) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/healthz", get(healthz))
        .route("/slow", get(slow))
        .with_state(state)
        .layer(middleware::from_fn_with_state(logger, request_logging))
}

async fn home(State(state): State<AppState>) -> Response {
    match state.counter.increment().await {
        Ok(visits) => (
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!(
                "{} (visit {}, served by {})\n",
                state.greeting, visits, state.hostname
            ),
        )
            .into_response(),
        Err(error) => {
            state.logger.event(json!({
                "event": "counter_error",
                "error": error.to_string(),
            }));
            (StatusCode::SERVICE_UNAVAILABLE, "visit counter unavailable").into_response()
        }
    }
}

async fn healthz() -> Response {
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], "ok").into_response()
}

async fn slow() -> StatusCode {
    tokio::time::sleep(Duration::from_secs(3)).await;
    StatusCode::OK
}

async fn request_logging(
    State(logger): State<JsonLogger>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let method = request.method().as_str().to_owned();
    let path = request.uri().path().to_owned();
    let started = Instant::now();

    let response = next.run(request).await;

    logger.event(json!({
        "method": method,
        "path": path,
        "status": response.status().as_u16(),
        "duration_ms": started.elapsed().as_millis() as u64,
    }));

    response
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::Value;
    use tower::ServiceExt;

    use super::{AppState, router};
    use crate::logging::JsonLogger;
    use crate::store::{StoreError, VisitCounter};

    #[derive(Clone)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct FakeCounter {
        visit: AtomicI64,
        calls: AtomicUsize,
        fail: AtomicBool,
    }

    impl FakeCounter {
        fn new() -> Self {
            Self {
                visit: AtomicI64::new(0),
                calls: AtomicUsize::new(0),
                fail: AtomicBool::new(false),
            }
        }

        fn failing() -> Self {
            let counter = Self::new();
            counter.fail.store(true, Ordering::SeqCst);
            counter
        }
    }

    #[async_trait]
    impl VisitCounter for FakeCounter {
        async fn increment(&self) -> Result<i64, StoreError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                return Err(StoreError::new("Redis unavailable"));
            }
            Ok(self.visit.fetch_add(1, Ordering::SeqCst) + 1)
        }

        async fn reset(&self) -> Result<(), StoreError> {
            self.visit.store(0, Ordering::SeqCst);
            Ok(())
        }
    }

    fn test_router(counter: Arc<dyn VisitCounter>) -> (axum::Router, Arc<Mutex<Vec<u8>>>) {
        let output = Arc::new(Mutex::new(Vec::new()));
        let logger = JsonLogger::new(Box::new(SharedWriter(output.clone())));
        let state = AppState {
            greeting: Arc::from("hello from tests"),
            hostname: Arc::from("test-host"),
            counter,
            logger: logger.clone(),
        };
        (router(state, logger), output)
    }

    async fn send(router: axum::Router, path: &str) -> (StatusCode, String) {
        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    fn log_lines(output: &Arc<Mutex<Vec<u8>>>) -> Vec<Value> {
        String::from_utf8(output.lock().unwrap().clone())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[tokio::test]
    async fn healthz_does_not_use_the_counter() {
        let counter = Arc::new(FakeCounter::failing());
        let (app, _) = test_router(counter.clone());

        let (status, body) = send(app, "/healthz").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "ok");
        assert_eq!(counter.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn home_increments_and_formats_the_response() {
        let counter = Arc::new(FakeCounter::new());
        let (app, _) = test_router(counter.clone());

        let (status, body) = send(app, "/").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "hello from tests (visit 1, served by test-host)\n");
    }

    #[tokio::test]
    async fn home_answers_503_when_the_counter_is_unavailable() {
        let counter = Arc::new(FakeCounter::failing());
        let (app, _) = test_router(counter);

        let (status, _) = send(app, "/").await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn request_log_is_one_json_object_per_request() {
        let (app, output) = test_router(Arc::new(FakeCounter::new()));

        let _ = send(app, "/healthz").await;

        let lines = log_lines(&output);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["method"], "GET");
        assert_eq!(lines[0]["path"], "/healthz");
        assert_eq!(lines[0]["status"], 200);
    }

    #[tokio::test]
    async fn slow_takes_three_seconds() {
        let (app, _) = test_router(Arc::new(FakeCounter::new()));

        let started = std::time::Instant::now();
        let (status, _) = send(app, "/slow").await;

        assert_eq!(status, StatusCode::OK);
        assert!(started.elapsed() >= std::time::Duration::from_secs(3));
    }
}
