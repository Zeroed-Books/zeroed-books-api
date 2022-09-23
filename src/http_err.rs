use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use tracing::error;

use crate::rate_limit::RateLimitResult;

#[derive(Serialize)]
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

pub enum ApiError {
    InternalServerError(InternalServerError),
    TooManyRequests(RateLimitResult),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::InternalServerError(inner) => inner.into_response(),
            Self::TooManyRequests(result) => result.into_response(),
        }
    }
}

impl From<InternalServerError> for ApiError {
    fn from(error: InternalServerError) -> Self {
        Self::InternalServerError(error)
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

        Self::InternalServerError(Default::default())
    }
}

pub type ApiResponse<T> = Result<T, ApiError>;

#[derive(Serialize)]
pub struct ErrorRep {
    pub message: String,
}
