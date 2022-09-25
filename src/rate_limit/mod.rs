mod redis;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use thiserror::Error;

use crate::http_err::{ErrorRep, InternalServerError};

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
    /// Determine if the rate limit has been exceeded for a specific resource.
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
    /// In the typical case, an [Ok] result containing a result describing the
    /// requestor's rate limit state is returned. An [Err] is returned if the
    /// rate limiter encounters an error while trying to determine if the
    /// request should be rate limited.
    #[deprecated = "Use `record_operation` for better error ergonomics."]
    fn is_limited(&self, key: &str, max_req_per_min: u64) -> anyhow::Result<RateLimitResult>;

    fn record_operation(&self, key: &str, max_req_per_min: u64) -> Result<(), RateLimitError>;
}

#[derive(Debug)]
#[deprecated = "Use `record_operation` for better error ergonomics."]
pub enum RateLimitResult {
    /// The rate limit has not been exceeded.
    NotLimited,
    /// The rate limit has been exceeded. Requests will be accepted again at the
    /// contained timestamp.
    LimitedUntil(chrono::DateTime<Utc>),
}

#[derive(Serialize)]
#[deprecated = "Use `record_operation` for better error ergonomics."]
pub struct RateLimitResponse {
    pub message: Option<String>,
}

impl From<RateLimitResult> for RateLimitResponse {
    fn from(result: RateLimitResult) -> Self {
        match result {
            RateLimitResult::LimitedUntil(_time) => Self {
                message: Some("Too many attempts. Please try again later.".to_string()),
            },
            RateLimitResult::NotLimited => Self { message: None },
        }
    }
}

impl IntoResponse for RateLimitResult {
    fn into_response(self) -> Response {
        if let Self::LimitedUntil(_time) = self {
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(RateLimitResponse::from(self)),
            )
                .into_response()
        } else {
            // A `RateLimitResult` will typically only be converted to a
            // response in a failure scenario, but if a non-limited result is
            // converted, we just respond with a simple success status code.
            StatusCode::OK.into_response()
        }
    }
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

                InternalServerError::default().into_response()
            }
        }
    }
}
