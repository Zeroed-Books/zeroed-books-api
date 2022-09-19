use std::net::IpAddr;

use rocket::{serde::json::Json, Route, State};
use semval::ValidatedFrom;
use sqlx::PgPool;
use tera::Tera;
use tracing::error;

use crate::{
    create_user,
    email::clients::EmailClient,
    http_err::{ApiResponse, InternalServerError},
    identities::queries::{postgres::PostgresQueries, PasswordResetQueries},
    passwords::Password,
    rate_limit::{RateLimitResult, RateLimiter},
    verify_email,
};

use super::{
    commands::{postgres::PostgresCommands, PasswordResetCommands, UserCommands},
    domain::password_resets::{NewPasswordReset, PasswordResetToken},
    queries,
};

pub mod reps;

pub fn routes() -> Vec<Route> {
    routes![
        create_password_reset,
        create_password_reset_request,
        create_user,
        verify_email
    ]
}

#[derive(Responder)]
pub enum ResetPasswordResponse {
    #[response(status = 200)]
    Ok(()),
    #[response(status = 400)]
    BadRequest(Json<reps::PasswordResetError>),
}

impl From<()> for ResetPasswordResponse {
    fn from(_: ()) -> Self {
        Self::Ok(())
    }
}

impl From<reps::PasswordResetError> for ResetPasswordResponse {
    fn from(response: reps::PasswordResetError) -> Self {
        Self::BadRequest(Json(response))
    }
}

#[post("/password-resets", data = "<reset_data>")]
async fn create_password_reset<'r>(
    client_ip: IpAddr,
    db: &State<PgPool>,
    rate_limiter: &State<Box<dyn RateLimiter>>,
    reset_data: Json<reps::PasswordReset<'r>>,
) -> ApiResponse<ResetPasswordResponse> {
    let rate_limit_key = format!("/identities/password-resets_post_{}", client_ip);
    match rate_limiter.is_limited(&rate_limit_key, 10) {
        Ok(RateLimitResult::NotLimited) => (),
        Ok(result @ RateLimitResult::LimitedUntil(_)) => return Err(result.into()),
        Err(error) => {
            error!(?error, "Failed to query rate limiter.");

            return Err(InternalServerError::default().into());
        }
    };

    let password = match Password::validated_from(reset_data.new_password) {
        Ok(password) => password,
        Err((_, context)) => {
            return Ok(reps::PasswordResetError::from(context).into());
        }
    };

    let queries = PostgresQueries(db);

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

            return Err(InternalServerError::default().into());
        }
    };

    let validated_token = match PasswordResetToken::validated_from(password_reset_data) {
        Ok(token) => token,
        Err((_, context)) => {
            return Ok(reps::PasswordResetError::from(context).into());
        }
    };

    let commands = PostgresCommands(db);
    match commands
        .reset_user_password(validated_token, password)
        .await
    {
        Ok(()) => Ok(().into()),
        Err(error) => {
            error!(?error, "Failed to change user's password.");

            Err(InternalServerError::default().into())
        }
    }
}

#[derive(Responder)]
pub enum CreatePasswordResetResponse<'r> {
    #[response(status = 200)]
    Ok(Json<reps::PasswordResetRequest<'r>>),
    #[response(status = 400)]
    BadRequest(Json<reps::PasswordResetRequestError>),
}

impl<'r> From<reps::PasswordResetRequest<'r>> for CreatePasswordResetResponse<'r> {
    fn from(response: reps::PasswordResetRequest<'r>) -> Self {
        Self::Ok(Json(response))
    }
}

impl From<reps::PasswordResetRequestError> for CreatePasswordResetResponse<'_> {
    fn from(response: reps::PasswordResetRequestError) -> Self {
        Self::BadRequest(Json(response))
    }
}

#[post("/password-reset-requests", data = "<reset_request>")]
async fn create_password_reset_request<'r>(
    client_ip: IpAddr,
    db: &State<PgPool>,
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

    let password_reset = match NewPasswordReset::validated_from(reset_request.email) {
        Ok(reset) => reset,
        Err((_, context)) => {
            return Ok(reps::PasswordResetRequestError::from(context).into());
        }
    };

    let commands = PostgresCommands(db);
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
