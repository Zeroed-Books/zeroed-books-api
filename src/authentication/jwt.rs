use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
    http::status::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::debug;

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

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for TokenClaims
where
    axum_jwks::Jwks: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = JwtError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let jwks = axum_jwks::Jwks::from_ref(state);
        let token = axum_jwks::Token::from_request_parts(parts, state)
            .await
            .map_err(|_| {
                debug!(
                    "Cannot extract token claims from request due to missing authentication token."
                );

                JwtError::Missing
            })?;

        let token_data = jwks.validate_claims(token.value()).map_err(|error| {
            debug!(?error, "Invalid authentication token received.");
            JwtError::Invalid
        })?;

        Ok(token_data.claims)
    }
}

pub enum JwtError {
    Invalid,
    Missing,
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
