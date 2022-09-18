// We often use a `new` constructor with required arguments to ensure that only
// structs with valid data can be created. A default implementation would avoid
// this benefit we get from the type system.
#![allow(clippy::new_without_default)]
#![deny(elided_lifetimes_in_paths)]

#[macro_use]
extern crate rocket;

use std::{convert::TryInto, net::IpAddr};

use chrono::Duration;
use email::clients::{EmailClient, Message};
use http_err::ApiError;
use identities::{
    domain::{
        email::EmailVerification,
        users::{NewUser, NewUserData},
    },
    models::email::{EmailPersistanceError, NewEmail, NewEmailVerification},
};
use models::NewUserModel;
use rate_limit::{RateLimitResult, RateLimiter};
use rocket::{
    response::Responder,
    serde::{json::Json, Deserialize, Serialize},
    State,
};
use semval::ValidatedFrom;
use sqlx::PgPool;
use tera::{Context, Tera};
use tracing::{error, trace};

use crate::http_err::InternalServerError;

pub mod authentication;
pub mod cli;
pub mod cors;
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

#[derive(Responder)]
pub enum CreateUserResponse {
    #[response(status = 201)]
    UserCreated(Json<identities::http::reps::NewUserResponse>),
    #[response(status = 400)]
    BadRequest(Json<identities::http::reps::NewUserValidationError>),
}

impl From<identities::http::reps::NewUserResponse> for CreateUserResponse {
    fn from(response: identities::http::reps::NewUserResponse) -> Self {
        Self::UserCreated(Json(response))
    }
}

impl From<identities::http::reps::NewUserValidationError> for CreateUserResponse {
    fn from(response: identities::http::reps::NewUserValidationError) -> Self {
        Self::BadRequest(Json(response))
    }
}

#[post("/users", data = "<new_user_data>")]
pub async fn create_user(
    db: &State<PgPool>,
    client_ip: IpAddr,
    email_client: &State<Box<dyn EmailClient>>,
    rate_limiter: &State<Box<dyn RateLimiter>>,
    templates: &State<Tera>,
    new_user_data: Json<identities::http::reps::NewUserRequest<'_>>,
) -> Result<CreateUserResponse, ApiError> {
    let rate_limit_key = format!("/users_post_{}", client_ip);
    match rate_limiter.is_limited(&rate_limit_key, 10) {
        Ok(RateLimitResult::NotLimited) => (),
        Ok(result @ RateLimitResult::LimitedUntil(_)) => return Err(result.into()),
        Err(err) => {
            error!(error = ?err, "Failed to query rate limiter.");

            return Err(InternalServerError::default().into());
        }
    };

    // TODO: Validate both password and email, so both sets of errors can be
    //       presented in the same response.

    let new_user = match NewUser::validated_from(NewUserData::from(new_user_data.0)) {
        Ok(user) => user,
        Err((_, context)) => {
            return Ok(identities::http::reps::NewUserValidationError::from(context).into())
        }
    };

    let user_model: models::NewUserModel = match (&new_user).try_into() {
        Ok(model) => model,
        Err(error) => {
            error!(?error, "Failed to convert new user to model.");

            return Err(InternalServerError::default().into());
        }
    };
    let email_model = NewEmail::for_user(new_user.id(), new_user.email());
    let new_email_id = email_model.id();

    let persistance_result = persist_new_user(db, user_model, email_model).await;

    if let Err(persistence_err) = persistance_result {
        match persistence_err {
            EmailPersistanceError::DuplicateEmail(_) => {
                let content = templates
                    .render("emails/duplicate.txt", &Context::new())
                    .expect("template failure");

                let message = Message {
                    to: new_user.email().address().to_owned(),
                    subject: "Duplicate Registration".to_owned(),
                    text: content,
                };

                match email_client.send(&message).await {
                    Ok(()) => {
                        return Ok(CreateUserResponse::UserCreated(Json(
                            identities::http::reps::NewUserResponse {
                                email: new_user.email().address().to_owned(),
                            },
                        )))
                    }
                    Err(e) => {
                        error!(
                            error = ?e,
                            "Failed to send duplicate registration email."
                        );

                        return Err(InternalServerError {
                            message: "Internal server error.".to_owned(),
                        }
                        .into());
                    }
                }
            }
            error => {
                error!(?error, "Failed to persist new user.");

                return Err(InternalServerError {
                    message: "Internal server error.".to_owned(),
                }
                .into());
            }
        }
    }

    let verification = EmailVerification::new();
    let verification_model = NewEmailVerification::new(new_email_id, &verification);

    let verification_save_result = verification_model.save(db).await;
    match verification_save_result {
        Ok(()) => (),
        Err(err) => {
            error!(
                error = ?err,
                "Failed to save email verification model."
            );

            return Err(InternalServerError::default().into());
        }
    };

    let mut verification_context = Context::new();
    verification_context.insert("token", verification.token());

    let content = templates
        .render("emails/verify.txt", &verification_context)
        .expect("template failure");

    let message = Message {
        to: new_user.email().address().to_owned(),
        subject: "Please Confirm your Email".to_owned(),
        text: content,
    };

    match email_client.send(&message).await {
        Ok(()) => (),
        Err(e) => {
            error!(error = ?e, "Failed to send verification email.");

            return Err(InternalServerError {
                message: "Internal server error.".to_owned(),
            }
            .into());
        }
    }

    Ok(CreateUserResponse::UserCreated(Json(
        identities::http::reps::NewUserResponse {
            email: new_user.email().address().to_owned(),
        },
    )))
}

async fn persist_new_user(
    conn: &PgPool,
    user: NewUserModel,
    email: NewEmail,
) -> Result<(), EmailPersistanceError> {
    let mut tx = conn.begin().await?;

    sqlx::query!(
        r#"
        INSERT INTO "user" (id, password)
        VALUES ($1, $2)
        "#,
        user.id,
        user.password_hash
    )
    .execute(&mut tx)
    .await?;

    email.save(&mut tx).await?;

    tx.commit().await?;

    Ok(())
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

#[derive(Responder)]
pub enum EmailVerificationResponse {
    #[response(status = 201)]
    Verified(Json<EmailVerified>),
    #[response(status = 400)]
    BadRequest(Json<VerificationError>),
}

#[post("/email-verifications", data = "<verification_request>")]
pub async fn verify_email(
    db: &State<PgPool>,
    verification_request: Json<EmailVerificationRequest>,
) -> Result<EmailVerificationResponse, ApiError> {
    let verification_result = mark_email_as_verified(&db, &verification_request.token).await;

    match verification_result {
        Ok(EmailVerificationResult::EmailVerified(address)) => {
            Ok(EmailVerificationResponse::Verified(Json(EmailVerified {
                email: address,
            })))
        }
        Ok(EmailVerificationResult::NotFound) => Ok(EmailVerificationResponse::BadRequest(Json(
            VerificationError {
                message: "The provided verification token is either invalid or has expired."
                    .to_string(),
            },
        ))),
        Err(err) => {
            error!(error = ?err, "Failed to verify email.");

            Err(InternalServerError {
                message: "Internal server error.".to_string(),
            }
            .into())
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
