use rocket::response::Responder;
use rocket::serde::json::Json;
use rocket::serde::Serialize;

use crate::rate_limit::RateLimitResult;

#[derive(Serialize)]
pub struct InternalServerError {
    pub message: String,
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
