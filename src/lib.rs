// We often use a `new` constructor with required arguments to ensure that only
// structs with valid data can be created. A default implementation would avoid
// this benefit we get from the type system.
#![allow(clippy::new_without_default)]
#![deny(elided_lifetimes_in_paths)]

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Duration;
use client_ip::ClientIp;
use http_err::{ApiError, ApiResponse};
use identities::services::{CreateUserResult, UserService};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, trace};

use crate::http_err::InternalServerError;

pub mod authentication;
pub mod cli;
pub mod client_ip;
mod email;
mod http_err;
mod identities;
pub mod ledger;
mod models;
pub mod passwords;
mod rate_limit;
mod server;

#[derive(Serialize)]
pub struct RegistrationError {
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<Vec<String>>,
}

pub enum CreateUserResponse {
    UserCreated(identities::http::reps::NewUserResponse),
    BadRequest(identities::http::reps::NewUserValidationError),
}

impl IntoResponse for CreateUserResponse {
    fn into_response(self) -> Response {
        match self {
            Self::UserCreated(user) => (StatusCode::CREATED, Json(user)).into_response(),
            Self::BadRequest(error) => (StatusCode::BAD_REQUEST, Json(error)).into_response(),
        }
    }
}

impl From<identities::http::reps::NewUserResponse> for CreateUserResponse {
    fn from(response: identities::http::reps::NewUserResponse) -> Self {
        Self::UserCreated(response)
    }
}

impl From<identities::http::reps::NewUserValidationError> for CreateUserResponse {
    fn from(response: identities::http::reps::NewUserValidationError) -> Self {
        Self::BadRequest(response)
    }
}

pub async fn create_user(
    ClientIp(client_ip): ClientIp,
    State(user_service): State<UserService>,
    Json(new_user_data): Json<identities::http::reps::NewUserRequest>,
) -> Result<CreateUserResponse, ApiError> {
    match user_service
        .create_user(&client_ip.to_string(), new_user_data.into())
        .await
    {
        Ok(CreateUserResult::Created(user)) => Ok(CreateUserResponse::UserCreated(user.into())),
        Ok(CreateUserResult::InvalidUser(context)) => {
            Ok(CreateUserResponse::BadRequest(context.into()))
        }
        Ok(CreateUserResult::RateLimited(result)) => Err(result.into()),
        Err(error) => {
            error!(?error, "Failed to create new user.");

            Err(InternalServerError::default().into())
        }
    }
}

#[derive(Deserialize)]
pub struct EmailVerificationRequest {
    token: String,
}

#[derive(Serialize)]
pub struct EmailVerified {
    email: String,
}

#[derive(Serialize)]
pub struct VerificationError {
    message: String,
}

pub enum EmailVerificationResponse {
    Verified(EmailVerified),
    BadRequest(VerificationError),
}

impl IntoResponse for EmailVerificationResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Verified(verification) => {
                (StatusCode::CREATED, Json(verification)).into_response()
            }
            Self::BadRequest(error) => (StatusCode::BAD_REQUEST, Json(error)).into_response(),
        }
    }
}

pub async fn verify_email(
    State(db): State<PgPool>,
    Json(verification_request): Json<EmailVerificationRequest>,
) -> ApiResponse<EmailVerificationResponse> {
    let verification_result = mark_email_as_verified(&db, &verification_request.token).await;

    match verification_result {
        Ok(EmailVerificationResult::EmailVerified(address)) => {
            Ok(EmailVerificationResponse::Verified(EmailVerified {
                email: address,
            }))
        }
        Ok(EmailVerificationResult::NotFound) => {
            Ok(EmailVerificationResponse::BadRequest(VerificationError {
                message: "The provided verification token is either invalid or has expired."
                    .to_string(),
            }))
        }
        Err(err) => {
            error!(error = ?err, "Failed to verify email.");

            Err(InternalServerError::default().into())
        }
    }
}

enum EmailVerificationResult {
    EmailVerified(String),
    NotFound,
}

async fn mark_email_as_verified(
    db: &PgPool,
    token: &str,
) -> Result<EmailVerificationResult, sqlx::Error> {
    let now = chrono::Utc::now();
    let expiration = now - Duration::days(1);

    trace!(%now, %expiration, "Verifying email address.");

    let verified_address = sqlx::query!(
        r#"
        WITH pending_verification_emails AS (
            SELECT email_id
            FROM email_verification
            WHERE token = $1 AND created_at > $2
        )
        UPDATE email
        SET verified_at = now()
        WHERE id = ANY(SELECT * FROM pending_verification_emails)
        RETURNING provided_address
        "#,
        token,
        expiration
    )
    .fetch_optional(db)
    .await?
    .map(|record| record.provided_address);

    match verified_address {
        Some(address) => {
            sqlx::query!(
                r#"
                DELETE FROM email_verification
                WHERE token = $1
                "#,
                token
            )
            .execute(db)
            .await?;

            Ok(EmailVerificationResult::EmailVerified(address))
        }
        None => Ok(EmailVerificationResult::NotFound),
    }
}
