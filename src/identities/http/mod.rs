use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use semval::ValidatedFrom;
use sqlx::PgPool;
use tera::Tera;
use tracing::error;

use crate::{
    client_ip::ClientIp,
    create_user,
    email::clients::EmailClient,
    http_err::{ApiError, ApiResponse},
    identities::queries::{postgres::PostgresQueries, PasswordResetQueries},
    passwords::Password,
    rate_limit::{RateLimitError, RateLimiter},
    server::AppState,
    verify_email,
};

use super::{
    commands::{postgres::PostgresCommands, PasswordResetCommands, UserCommands},
    domain::password_resets::{NewPasswordReset, PasswordResetToken},
    queries,
};

pub mod reps;

pub fn routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .with_state(app_state)
        .route("/email-verifications", post(verify_email))
        .route(
            "/password-reset-requests",
            post(create_password_reset_request),
        )
        .route("/password-resets", post(create_password_reset))
        .route("/users", post(create_user))
}

pub enum ResetPasswordResponse {
    Ok(()),
    BadRequest(reps::PasswordResetError),
}

impl IntoResponse for ResetPasswordResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(_) => StatusCode::OK.into_response(),
            Self::BadRequest(error) => (StatusCode::BAD_REQUEST, Json(error)).into_response(),
        }
    }
}

impl From<()> for ResetPasswordResponse {
    fn from(_: ()) -> Self {
        Self::Ok(())
    }
}

impl From<reps::PasswordResetError> for ResetPasswordResponse {
    fn from(response: reps::PasswordResetError) -> Self {
        Self::BadRequest(response)
    }
}

async fn create_password_reset(
    ClientIp(client_ip): ClientIp,
    State(db): State<PgPool>,
    State(rate_limiter): State<Arc<dyn RateLimiter>>,
    Json(reset_data): Json<reps::PasswordReset>,
) -> ApiResponse<ResetPasswordResponse> {
    let rate_limit_key = format!("/identities/password-resets_post_{}", client_ip);
    match rate_limiter.record_operation(&rate_limit_key, 10) {
        Ok(_) => (),
        Err(result @ RateLimitError::LimitedUntil(_)) => return Err(result.into()),
        Err(error) => {
            error!(?error, "Failed to query rate limiter.");

            return Err(ApiError::InternalServerError);
        }
    };

    let password = match Password::validated_from(reset_data.new_password.as_str()) {
        Ok(password) => password,
        Err((_, context)) => {
            return Ok(reps::PasswordResetError::from(context).into());
        }
    };

    let queries = PostgresQueries(&db);

    let password_reset_data = match queries
        .get_password_reset(reset_data.token.to_owned())
        .await
    {
        Ok(data) => data,
        Err(error @ queries::PasswordResetError::NotFound) => {
            return Ok(reps::PasswordResetError::from(error).into())
        }
        Err(error) => {
            error!(?error, "Failed to query for password reset.");

            return Err(ApiError::InternalServerError);
        }
    };

    let validated_token = match PasswordResetToken::validated_from(password_reset_data) {
        Ok(token) => token,
        Err((_, context)) => {
            return Ok(reps::PasswordResetError::from(context).into());
        }
    };

    let commands = PostgresCommands(&db);
    match commands
        .reset_user_password(validated_token, password)
        .await
    {
        Ok(()) => Ok(().into()),
        Err(error) => {
            error!(?error, "Failed to change user's password.");

            Err(ApiError::InternalServerError)
        }
    }
}

pub enum CreatePasswordResetResponse {
    Ok(reps::PasswordResetRequest),
    BadRequest(reps::PasswordResetRequestError),
}

impl IntoResponse for CreatePasswordResetResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Self::BadRequest(error) => (StatusCode::BAD_REQUEST, Json(error)).into_response(),
        }
    }
}

impl From<reps::PasswordResetRequest> for CreatePasswordResetResponse {
    fn from(response: reps::PasswordResetRequest) -> Self {
        Self::Ok(response)
    }
}

impl From<reps::PasswordResetRequestError> for CreatePasswordResetResponse {
    fn from(response: reps::PasswordResetRequestError) -> Self {
        Self::BadRequest(response)
    }
}

async fn create_password_reset_request(
    ClientIp(client_ip): ClientIp,
    State(db): State<PgPool>,
    State(mailer): State<Arc<dyn EmailClient>>,
    State(rate_limiter): State<Arc<dyn RateLimiter>>,
    State(tera): State<Tera>,
    Json(reset_request): Json<reps::PasswordResetRequest>,
) -> ApiResponse<CreatePasswordResetResponse> {
    let rate_limit_key = format!("/identities/password-reset-requests_post_{}", client_ip);
    match rate_limiter.record_operation(&rate_limit_key, 10) {
        Ok(_) => (),
        Err(result @ RateLimitError::LimitedUntil(_)) => return Err(result.into()),
        Err(error) => {
            error!(?error, "Failed to query rate limiter.");

            return Err(ApiError::InternalServerError);
        }
    };

    let password_reset = match NewPasswordReset::validated_from(reset_request.email.as_ref()) {
        Ok(reset) => reset,
        Err((_, context)) => {
            return Ok(reps::PasswordResetRequestError::from(context).into());
        }
    };

    let commands = PostgresCommands(&db);
    match commands
        .create_reset_token(password_reset, mailer.as_ref(), &tera)
        .await
    {
        Ok(()) => Ok(reset_request.into()),
        Err(error) => {
            error!(?error, "Failed to save password reset token.");

            Err(ApiError::InternalServerError)
        }
    }
}
