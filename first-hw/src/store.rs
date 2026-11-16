use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use redis::AsyncCommands;

const VISITS_KEY: &str = "visits";
const OPERATION_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub struct StoreError(String);

impl StoreError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StoreError {}

/// The Redis operations the HTTP layer needs. Keeping this behind a trait lets
/// the handlers be tested without a live Redis.
#[async_trait]
pub trait VisitCounter: Send + Sync {
    async fn increment(&self) -> Result<i64, StoreError>;
    async fn reset(&self) -> Result<(), StoreError>;
}

#[derive(Clone)]
pub struct RedisStore {
    client: redis::Client,
}

impl RedisStore {
    pub fn new(redis_url: &str) -> Result<Self, StoreError> {
        let client = redis::Client::open(redis_url)
            .map_err(|error| StoreError::new(format!("parse Redis URL: {error}")))?;

        Ok(Self { client })
    }

    async fn connection(&self) -> Result<redis::aio::MultiplexedConnection, StoreError> {
        match tokio::time::timeout(
            OPERATION_TIMEOUT,
            self.client.get_multiplexed_async_connection(),
        )
        .await
        {
            Ok(Ok(connection)) => Ok(connection),
            Ok(Err(error)) => Err(StoreError::new(format!("connect Redis: {error}"))),
            Err(_) => Err(StoreError::new("connect Redis: timed out")),
        }
    }
}

#[async_trait]
impl VisitCounter for RedisStore {
    async fn increment(&self) -> Result<i64, StoreError> {
        let mut connection = self.connection().await?;

        let value: i64 = tokio::time::timeout(OPERATION_TIMEOUT, connection.incr(VISITS_KEY, 1))
            .await
            .map_err(|_| StoreError::new("increment visit counter: timed out"))?
            .map_err(|error| StoreError::new(format!("increment visit counter: {error}")))?;

        Ok(value)
    }

    async fn reset(&self) -> Result<(), StoreError> {
        let mut connection = self.connection().await?;

        let _: () = tokio::time::timeout(OPERATION_TIMEOUT, connection.set(VISITS_KEY, 0))
            .await
            .map_err(|_| StoreError::new("reset visit counter: timed out"))?
            .map_err(|error| StoreError::new(format!("reset visit counter: {error}")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::RedisStore;

    #[test]
    fn rejects_a_url_without_a_scheme() {
        assert!(RedisStore::new("not-a-url").is_err());
    }
}
