use axum::{http::status::StatusCode, response::IntoResponse, Json};
use axum_jwks::{ParseTokenClaims, TokenError};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, Serialize)]
pub struct TokenClaims {
    iss: String,
    sub: String,
}

impl TokenClaims {
    pub fn iss(&self) -> &str {
        &self.iss
    }

    pub fn sub(&self) -> &str {
        &self.sub
    }

    /// Get the ID of the user that the token claims represent.
    ///
    /// This is the user who made the request.
    pub fn user_id(&self) -> &str {
        &self.sub
    }
}

impl ParseTokenClaims for TokenClaims {
    type Rejection = JwtError;
}

pub enum JwtError {
    Invalid,
    Missing,
}

impl From<TokenError> for JwtError {
    fn from(value: TokenError) -> Self {
        match value {
            TokenError::Missing => Self::Missing,
            _ => Self::Invalid,
        }
    }
}

impl IntoResponse for JwtError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            Self::Invalid => (StatusCode::UNAUTHORIZED, "Invalid authentication token."),
            Self::Missing => (
                StatusCode::UNAUTHORIZED,
                "No authentication token provided.",
            ),
        };

        let body = Json(json!({
            "error": message,
        }));

        (status, body).into_response()
    }
}
