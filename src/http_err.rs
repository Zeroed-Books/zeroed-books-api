use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use tracing::error;
use validator::ValidationErrors;

#[derive(Debug)]
pub enum ApiError {
    BadRequestReason(String),

    InternalServerError,

    ValidationError(ValidationErrors),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequestReason(reason) => {
                (StatusCode::BAD_REQUEST, Json(ErrorRep { message: reason })).into_response()
            }
            Self::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorRep {
                    message: "Internal server error.".to_owned(),
                }),
            )
                .into_response(),
            Self::ValidationError(error) => (StatusCode::BAD_REQUEST, Json(error)).into_response(),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        error!(?error, "Received error.");

        Self::InternalServerError
    }
}

impl From<ValidationErrors> for ApiError {
    fn from(value: ValidationErrors) -> Self {
        Self::ValidationError(value)
    }
}

pub type ApiResponse<T> = Result<T, ApiError>;

#[derive(Serialize)]
pub struct ErrorRep {
    pub message: String,
}
