// We often use a `new` constructor with required arguments to ensure that only
// structs with valid data can be created. A default implementation would avoid
// this benefit we get from the type system.
#![allow(clippy::new_without_default)]
#![deny(elided_lifetimes_in_paths)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate rocket;

use std::net::IpAddr;

use chrono::Duration;
use diesel::{insert_into, RunQueryDsl};
use email::clients::{EmailClient, Message};
use http_err::ApiError;
use identities::{
    domain::{
        email::{Email, EmailVerification},
        users::NewUser,
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
use rocket_sync_db_pools::database;
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
mod rate_limit;
pub mod schema;
mod server;

#[database("postgres")]
pub struct PostgresConn(diesel::PgConnection);

#[derive(Deserialize)]
pub struct NewUserRequest<'r> {
    email: &'r str,
    password: &'r str,
}

#[derive(Serialize)]
pub struct NewUserResponse {
    email: String,
}

#[derive(Serialize)]
pub struct RegistrationError {
    email: Option<Vec<String>>,
}

#[derive(Responder)]
pub enum CreateUserResponse {
    #[response(status = 201)]
    UserCreated(Json<NewUserResponse>),
    #[response(status = 400)]
    BadRequest(Json<RegistrationError>),
}

#[post("/users", data = "<new_user_data>")]
pub async fn create_user(
    db: PostgresConn,
    client_ip: IpAddr,
    email_client: &State<Box<dyn EmailClient>>,
    rate_limiter: &State<Box<dyn RateLimiter>>,
    templates: &State<Tera>,
    new_user_data: Json<NewUserRequest<'_>>,
) -> Result<CreateUserResponse, ApiError> {
    let rate_limit_key = format!("/users_post_{}", client_ip);
    match rate_limiter.is_limited(&rate_limit_key, 10) {
        Ok(RateLimitResult::NotLimited) => (),
        Ok(result @ RateLimitResult::LimitedUntil(_)) => return Err(result.into()),
        Err(err) => {
            error!(error = ?err, "Failed to query rate limiter.");

            return Err(InternalServerError {
                message: "Internal server error.".to_string(),
            }
            .into());
        }
    };

    let new_user = match NewUser::new(new_user_data.password) {
        Ok(user) => user,
        Err(error) => {
            error!(?error, "Failed to create new user.");

            return Err(InternalServerError {
                message: "Internal server error.".to_string(),
            }
            .into());
        }
    };

    let email = match Email::parse(new_user_data.email) {
        Ok(parsed) => parsed,
        Err(_) => {
            return Ok(CreateUserResponse::BadRequest(Json(RegistrationError {
                email: Some(vec!["Please enter a valid email address.".to_string()]),
            })));
        }
    };

    let email_model = NewEmail::for_user(new_user.id(), &email);
    let new_email_id = email_model.id();

    let persistance_result = db
        .run(|c| persist_new_user(c, new_user.into(), email_model))
        .await;

    if let Err(persistence_err) = persistance_result {
        match persistence_err {
            EmailPersistanceError::DuplicateEmail(_) => {
                let content = templates
                    .render("emails/duplicate.txt", &Context::new())
                    .expect("template failure");

                let message = Message {
                    to: email.provided_address().to_string(),
                    subject: "Duplicate Registration".to_owned(),
                    text: content,
                };

                match email_client.send(&message).await {
                    Ok(()) => {
                        return Ok(CreateUserResponse::UserCreated(Json(NewUserResponse {
                            email: email.provided_address().to_string(),
                        })))
                    }
                    Err(e) => {
                        error!(
                            error = e.as_ref(),
                            "Failed to send duplicate registration email."
                        );

                        return Err(InternalServerError {
                            message: "Internal server error.".to_owned(),
                        }
                        .into());
                    }
                }
            }
            _ => {
                // TODO: Logging.
                return Err(InternalServerError {
                    message: "Internal server error.".to_owned(),
                }
                .into());
            }
        }
    }

    let verification = EmailVerification::new();
    let verification_model = NewEmailVerification::new(new_email_id, &verification);

    let verification_save_result = db.run(move |conn| verification_model.save(conn)).await;
    match verification_save_result {
        Ok(()) => (),
        Err(err) => {
            error!(
                error = ?err,
                "Failed to save email verification model."
            );

            return Err(InternalServerError {
                message: "Internal server error.".to_owned(),
            }
            .into());
        }
    };

    let mut verification_context = Context::new();
    verification_context.insert("token", verification.token());

    let content = templates
        .render("emails/verify.txt", &verification_context)
        .expect("template failure");

    let message = Message {
        to: email.provided_address().to_string(),
        subject: "Please Confirm your Email".to_owned(),
        text: content,
    };

    match email_client.send(&message).await {
        Ok(()) => (),
        Err(e) => {
            error!(error = e.as_ref(), "Failed to send verification email.");

            return Err(InternalServerError {
                message: "Internal server error.".to_owned(),
            }
            .into());
        }
    }

    Ok(CreateUserResponse::UserCreated(Json(NewUserResponse {
        email: email.provided_address().to_string(),
    })))
}

fn persist_new_user(
    conn: &diesel::PgConnection,
    user: NewUserModel,
    email: NewEmail,
) -> Result<(), EmailPersistanceError> {
    use crate::schema::user;

    conn.build_transaction().run(|| {
        insert_into(user::table).values(&user).execute(conn)?;

        email.save(conn)
    })
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
    db: PostgresConn,
    verification_request: Json<EmailVerificationRequest>,
) -> Result<EmailVerificationResponse, ApiError> {
    let verification_result = db
        .run(move |c| mark_email_as_verified(c, &verification_request.token))
        .await;

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

fn mark_email_as_verified(
    conn: &diesel::PgConnection,
    token: &str,
) -> Result<EmailVerificationResult, diesel::result::Error> {
    use crate::schema::{email, email_verification};
    use diesel::prelude::*;

    let now = chrono::Utc::now();
    let expiration = now - Duration::days(1);

    trace!(%now, %expiration, "Verifying email address.");

    let verification = email_verification::table.filter(
        email_verification::token
            .eq(token)
            .and(email_verification::created_at.gt(expiration)),
    );

    let verified_address: Result<String, diesel::result::Error> = diesel::update(email::table)
        .set(email::verified_at.eq(diesel::dsl::now))
        .filter(email::id.eq_any(verification.select(email_verification::email_id)))
        .returning(email::provided_address)
        .get_result(conn);

    match verified_address {
        Ok(address) => {
            let token_delete = diesel::delete(email_verification::table)
                .filter(email_verification::token.eq(token))
                .execute(conn);

            match token_delete {
                Ok(_) => Ok(EmailVerificationResult::EmailVerified(address)),
                Err(err) => Err(err),
            }
        }
        Err(diesel::NotFound) => Ok(EmailVerificationResult::NotFound),
        Err(err) => Err(err),
    }
}
