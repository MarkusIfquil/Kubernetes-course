use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

/// Writes one JSON object per line to a shared writer.
///
/// The production writer is stdout. A shared writer is used so tests can
/// capture the exact bytes that would have reached the container log.
#[derive(Clone)]
pub struct JsonLogger {
    out: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl JsonLogger {
    pub fn stdout() -> Self {
        Self::new(Box::new(io::stdout()))
    }

    pub fn new(out: Box<dyn Write + Send>) -> Self {
        Self {
            out: Arc::new(Mutex::new(out)),
        }
    }

    pub fn event(&self, event: Value) {
        let line = serde_json::to_string(&event)
            .unwrap_or_else(|_| json!({"event": "logging_error"}).to_string());

        if let Ok(mut out) = self.out.lock() {
            let _ = writeln!(out, "{line}");
            let _ = out.flush();
        }
    }
}
