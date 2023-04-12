use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("internal server error")]
    InternalServerError,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            Self::BadRequest(reason) => (StatusCode::BAD_REQUEST, ErrorRep { message: reason }),
            Self::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorRep {
                    message: "Internal server error.".to_owned(),
                },
            ),
        };

        (status, Json(body)).into_response()
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
