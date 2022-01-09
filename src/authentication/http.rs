use std::net::IpAddr;

use anyhow::Result;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use rocket::{
    http::{Cookie, CookieJar},
    serde::json::Json,
    Route, State,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use crate::{
    http_err::{ApiError, InternalServerError},
    rate_limit::{RateLimitResult, RateLimiter},
    PostgresConn,
};

use super::{domain::session::Session, models::User};

pub fn routes() -> Vec<Route> {
    routes![create_cookie_session]
}

#[derive(Deserialize)]
struct EmailPasswordPair {
    email: String,
    password: String,
}

#[derive(Serialize)]
pub struct SessionCreationError {
    message: String,
}

#[derive(Responder)]
pub enum CreateSessionResponse {
    #[response(status = 201)]
    Created(()),
    #[response(status = 400)]
    BadRequest(Json<SessionCreationError>),
}

#[post("/cookie-sessions", data = "<credentials>")]
async fn create_cookie_session(
    db: PostgresConn,
    client_ip: IpAddr,
    cookies: &CookieJar<'_>,
    rate_limiter: &State<Box<dyn RateLimiter>>,
    credentials: Json<EmailPasswordPair>,
) -> Result<CreateSessionResponse, ApiError> {
    let rate_limit_key = format!(
        "/authentication/cookie-sessions_post_{}",
        client_ip.to_string()
    );
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

    let user_email = credentials.email.clone();
    let user_query = db.run(move |conn| User::by_email(conn, &user_email)).await;
    let user_model = match user_query {
        Ok(user) => user,
        Err(error) => {
            error!(?error, "Error finding user by email.");

            return Err(InternalServerError {
                message: "Internal server error.".to_string(),
            }
            .into());
        }
    };

    let parsed_hash = match PasswordHash::new(&user_model.password_hash) {
        Ok(hash) => hash,
        Err(error) => {
            error!(?error, "Invalid password hash received.");

            return Err(InternalServerError {
                message: "Internal server error.".to_string(),
            }
            .into());
        }
    };

    let argon2 = Argon2::default();
    match argon2.verify_password(credentials.password.as_bytes(), &parsed_hash) {
        Ok(()) => {
            debug!(user_id = %user_model.id, "Validated user credentials.");

            let session = Session::new_for_user(user_model.id);
            let serialized_session = session.serialized()?;
            let session_cookie = Cookie::new("session", serialized_session);

            // TODO: Secure cookie when running under HTTPS
            cookies.add_private(session_cookie);

            Ok(CreateSessionResponse::Created(()))
        }
        Err(password_hash::Error::Password) => Ok(CreateSessionResponse::BadRequest(Json(
            SessionCreationError {
                message: "Invalid email or password.".to_string(),
            },
        ))),
        Err(error) => {
            error!(?error, "Failed to compare password and hash.");

            Err(InternalServerError::default().into())
        }
    }
}
