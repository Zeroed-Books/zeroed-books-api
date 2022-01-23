use rocket::response::Responder;
use rocket::serde::json::Json;
use rocket::serde::Serialize;
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

impl From<InternalServerError> for ApiError {
    fn from(error: InternalServerError) -> Self {
        Self::InternalServerError(Json(error))
    }
}

#[derive(Responder)]
pub enum ApiError {
    TooManyRequests(RateLimitResult),
    #[response(status = 500)]
    InternalServerError(Json<InternalServerError>),
}

impl From<RateLimitResult> for ApiError {
    fn from(result: RateLimitResult) -> Self {
        Self::TooManyRequests(result)
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        error!(?error, "Received error.");

        Self::InternalServerError(Json(InternalServerError {
            message: "Internal server error.".to_string(),
        }))
    }
}

#[derive(Serialize)]
pub struct ErrorRep {
    pub message: String,
}
