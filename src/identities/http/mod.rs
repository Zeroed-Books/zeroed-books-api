use std::net::IpAddr;

use rocket::{serde::json::Json, Route, State};
use semval::ValidatedFrom;
use tera::Tera;
use tracing::error;

use crate::{
    create_user,
    email::clients::EmailClient,
    http_err::{ApiResponse, InternalServerError},
    rate_limit::{RateLimitResult, RateLimiter},
    verify_email, PostgresConn,
};

use super::{
    commands::{postgres::PostgresCommands, PasswordResetCommands},
    domain::password_resets::PasswordReset,
};

pub mod reps;

pub fn routes() -> Vec<Route> {
    routes![create_password_reset_request, create_user, verify_email]
}

#[derive(Responder)]
pub enum CreatePasswordResetResponse<'r> {
    #[response(status = 200)]
    Ok(Json<reps::PasswordResetRequest<'r>>),
    #[response(status = 400)]
    BadRequest(Json<reps::PasswordResetError>),
}

impl<'r> From<reps::PasswordResetRequest<'r>> for CreatePasswordResetResponse<'r> {
    fn from(response: reps::PasswordResetRequest<'r>) -> Self {
        Self::Ok(Json(response))
    }
}

impl From<reps::PasswordResetError> for CreatePasswordResetResponse<'_> {
    fn from(response: reps::PasswordResetError) -> Self {
        Self::BadRequest(Json(response))
    }
}

#[post("/password-reset-requests", data = "<reset_request>")]
async fn create_password_reset_request<'r>(
    client_ip: IpAddr,
    db: PostgresConn,
    mailer: &State<Box<dyn EmailClient>>,
    rate_limiter: &State<Box<dyn RateLimiter>>,
    tera: &State<Tera>,
    reset_request: Json<reps::PasswordResetRequest<'r>>,
) -> ApiResponse<CreatePasswordResetResponse<'r>> {
    let rate_limit_key = format!("/identities/password-reset-requests_post_{}", client_ip);
    match rate_limiter.is_limited(&rate_limit_key, 10) {
        Ok(RateLimitResult::NotLimited) => (),
        Ok(result @ RateLimitResult::LimitedUntil(_)) => return Err(result.into()),
        Err(error) => {
            error!(?error, "Failed to query rate limiter.");

            return Err(InternalServerError::default().into());
        }
    };

    let password_reset = match PasswordReset::validated_from(reset_request.email) {
        Ok(reset) => reset,
        Err((_, context)) => {
            return Ok(reps::PasswordResetError::from(context).into());
        }
    };

    let commands = PostgresCommands(&db);
    match commands
        .create_reset_token(password_reset, mailer.as_ref(), tera)
        .await
    {
        Ok(()) => Ok(reset_request.0.into()),
        Err(error) => {
            error!(?error, "Failed to save password reset token.");

            Err(InternalServerError::default().into())
        }
    }
}
