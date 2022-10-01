mod redis;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::http_err::{ApiError, ErrorRep};

pub use self::redis::RedisRateLimiter;

#[derive(Debug, Error)]
pub enum RateLimitError {
    #[error("rate limited until {0}")]
    LimitedUntil(DateTime<Utc>),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// A requests-per-minute definition of a rate limiter.
pub trait RateLimiter: Send + Sync {
    /// Attempt to record an instance of a specific operation, failing if the
    /// specified rate limit is exceeded.
    ///
    /// # Arguments
    ///
    /// * `key` - A unique key for the resource being rate limited. In the
    ///   context of a web request, this should encapsulate the request path and
    ///   method, as well as the actor making the request.
    /// * `max_req_per_min` - The maximum number of requests allowed in a given
    ///   minute.
    ///
    /// # Returns
    ///
    /// A [Result] with the [Ok] variant indicating the operation was recorded
    /// and is permitted, and the [Err] variant describing why the request
    /// failed.
    fn record_operation(&self, key: &str, max_req_per_min: u64) -> Result<(), RateLimitError>;
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        match self {
            Self::LimitedUntil(_) => (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorRep {
                    message: "Too many requests. Please try again later".to_owned(),
                }),
            )
                .into_response(),
            Self::Other(error) => {
                tracing::error!(?error, "Unhandled rate limiting error.");

                ApiError::InternalServerError.into_response()
            }
        }
    }
}
