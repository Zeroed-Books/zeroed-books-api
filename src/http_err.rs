use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;
use tracing::error;

use crate::rate_limit::{RateLimitError, RateLimitResult};

#[derive(Serialize)]
#[deprecated = "Use `ApiError::InternalServerError` directly."]
pub struct InternalServerError {
    pub message: String,
}

impl Default for InternalServerError {
    fn default() -> Self {
        Self {
            message: "Internal server error.".to_string(),
        }
    }
}

impl IntoResponse for InternalServerError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
    }
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("internal server error")]
    InternalServerError,
    #[error(transparent)]
    RateLimited(#[from] RateLimitError),
    #[error("rate limited")]
    #[deprecated]
    TooManyRequests(RateLimitResult),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::InternalServerError => internal_server_error_response(),
            Self::RateLimited(error) => rate_limit_error_to_response(error),
            Self::TooManyRequests(result) => result.into_response(),
        }
    }
}

fn internal_server_error_response() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorRep {
            message: "Internal server error.".to_owned(),
        }),
    )
        .into_response()
}

fn rate_limit_error_to_response(error: RateLimitError) -> Response {
    match error {
        RateLimitError::LimitedUntil(_) => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorRep {
                message: "Too many requests. Please try again later".to_owned(),
            }),
        )
            .into_response(),
        RateLimitError::Other(error) => {
            error!(?error, "Unhandled rate limiting error.");

            InternalServerError::default().into_response()
        }
    }
}

impl From<InternalServerError> for ApiError {
    fn from(_: InternalServerError) -> Self {
        Self::InternalServerError
    }
}

impl From<RateLimitResult> for ApiError {
    fn from(result: RateLimitResult) -> Self {
        Self::TooManyRequests(result)
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        error!(?error, "Received error.");

        Self::InternalServerError
    }
}

pub type ApiResponse<T> = Result<T, ApiError>;

#[derive(Serialize)]
pub struct ErrorRep {
    pub message: String,
}
