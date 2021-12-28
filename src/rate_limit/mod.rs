mod redis;

use std::error::Error;

use chrono::Utc;
use rocket::{
    http::Status,
    response::Responder,
    serde::{json::Json, Serialize},
    Response,
};

pub use self::redis::RedisRateLimiter;

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
    fn is_limited(
        &self,
        key: &str,
        max_req_per_min: u64,
    ) -> Result<RateLimitResult, Box<dyn Error>>;
}

#[derive(Debug)]
pub enum RateLimitResult {
    /// The rate limit has not been exceeded.
    NotLimited,
    /// The rate limit has been exceeded. Requests will be accepted again at the
    /// contained timestamp.
    LimitedUntil(chrono::DateTime<Utc>),
}

#[derive(Serialize)]
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

impl<'r, 'o: 'r> Responder<'r, 'o> for RateLimitResult {
    fn respond_to(self, request: &'r rocket::Request<'_>) -> rocket::response::Result<'o> {
        let response_status = match self {
            Self::LimitedUntil(_time) => Status::TooManyRequests,
            _ => Status::Ok,
        };
        let response_data: RateLimitResponse = self.into();

        Response::build_from(Json(response_data).respond_to(request)?)
            .status(response_status)
            .ok()
    }
}
