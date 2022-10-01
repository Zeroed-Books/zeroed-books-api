use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;
use tracing::error;

use crate::rate_limit::RateLimitError;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("internal server error")]
    InternalServerError,
    #[error(transparent)]
    RateLimited(#[from] RateLimitError),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::InternalServerError => internal_server_error_response(),
            Self::RateLimited(error) => rate_limit_error_to_response(error),
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

            ApiError::InternalServerError.into_response()
        }
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
