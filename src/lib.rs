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
use client_ip::ClientIp;
use http_err::{ApiError, ApiResponse};
use identities::services::{CreateUserError, EmailService, UserService};
use repos::EmailVerificationError;
use serde::{Deserialize, Serialize};
use tracing::error;

pub mod authentication;
pub mod cli;
pub mod client_ip;
mod database;
mod email;
mod http_err;
mod identities;
pub mod ledger;
mod models;
pub mod passwords;
mod rate_limit;
mod repos;
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
        Ok(user) => Ok(CreateUserResponse::UserCreated(user.into())),
        Err(CreateUserError::InvalidUser(context)) => {
            Ok(CreateUserResponse::BadRequest(context.into()))
        }
        Err(CreateUserError::RateLimited(result)) => Err(result.into()),
        Err(error) => {
            error!(?error, "Failed to create new user.");

            Err(ApiError::InternalServerError)
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
    State(email_service): State<EmailService>,
    Json(verification_request): Json<EmailVerificationRequest>,
) -> ApiResponse<EmailVerificationResponse> {
    let verification_result = email_service
        .verify_email(&verification_request.token)
        .await;

    match verification_result {
        Ok(address) => Ok(EmailVerificationResponse::Verified(EmailVerified {
            email: address,
        })),
        Err(EmailVerificationError::InvalidToken) => {
            Ok(EmailVerificationResponse::BadRequest(VerificationError {
                message: "The provided verification token is either invalid or has expired."
                    .to_string(),
            }))
        }
        Err(err) => {
            error!(error = ?err, "Failed to verify email.");

            Err(ApiError::InternalServerError)
        }
    }
}
