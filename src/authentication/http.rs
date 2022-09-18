use std::net::IpAddr;

use anyhow::Result;
use rocket::{
    http::{Cookie, CookieJar},
    serde::json::Json,
    Route, State,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{debug, error};
use uuid::Uuid;

use crate::{
    http_err::{ApiError, InternalServerError},
    passwords,
    rate_limit::{RateLimitResult, RateLimiter},
};

use super::{domain::session::Session, models::User};

pub fn routes() -> Vec<Route> {
    routes![create_cookie_session, get_user_info]
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
    db: &State<PgPool>,
    client_ip: IpAddr,
    cookies: &CookieJar<'_>,
    rate_limiter: &State<Box<dyn RateLimiter>>,
    credentials: Json<EmailPasswordPair>,
) -> Result<CreateSessionResponse, ApiError> {
    let rate_limit_key = format!("/authentication/cookie-sessions_post_{}", client_ip);
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
    let user_query = User::by_email(db, &user_email).await;
    let user_model = match user_query {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Ok(CreateSessionResponse::BadRequest(Json(
                SessionCreationError {
                    message: "Invalid email or password.".to_string(),
                },
            )))
        }
        Err(error) => {
            error!(?error, "Error finding user by email.");

            return Err(InternalServerError {
                message: "Internal server error.".to_string(),
            }
            .into());
        }
    };

    let parsed_hash = match passwords::Hash::from_hash_str(&user_model.password_hash) {
        Ok(hash) => hash,
        Err(error) => {
            error!(?error, "Invalid password hash received from model.");

            return Err(InternalServerError::default().into());
        }
    };

    match parsed_hash.matches_raw_password(&credentials.password) {
        Ok(true) => {
            debug!(user_id = %user_model.id, "Validated user credentials.");

            let session = Session::new_for_user(user_model.id);
            let serialized_session = session.serialized()?;
            let session_cookie = Cookie::new("session", serialized_session);

            cookies.add_private(session_cookie);

            Ok(CreateSessionResponse::Created(()))
        }
        Ok(false) => Ok(CreateSessionResponse::BadRequest(Json(
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

#[derive(Serialize)]
pub struct UserInfo {
    pub user_id: Uuid,
}

#[get("/me")]
async fn get_user_info(session: Session) -> Json<UserInfo> {
    Json(UserInfo {
        user_id: session.user_id(),
    })
}
