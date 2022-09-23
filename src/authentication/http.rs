use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::{cookie::Cookie, PrivateCookieJar};
use cookie::time::Duration;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{debug, error};
use uuid::Uuid;

use crate::{
    client_ip::ClientIp,
    http_err::{ApiResponse, InternalServerError},
    passwords,
    rate_limit::{RateLimitResult, RateLimiter},
    server::AppState,
};

use super::{domain::session::Session, models::User};

pub fn routes(app_state: AppState) -> Router<AppState> {
    Router::with_state(app_state)
        .route("/cookie-sessions", post(create_cookie_session))
        .route("/me", get(get_user_info))
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

pub enum CreateSessionResponse {
    Created(PrivateCookieJar),
    BadRequest(SessionCreationError),
}

impl IntoResponse for CreateSessionResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Created(cookie_jar) => (cookie_jar, StatusCode::CREATED).into_response(),
            Self::BadRequest(error) => (StatusCode::BAD_REQUEST, Json(error)).into_response(),
        }
    }
}

async fn create_cookie_session(
    State(db): State<PgPool>,
    ClientIp(client_ip): ClientIp,
    cookies: PrivateCookieJar,
    State(rate_limiter): State<Arc<dyn RateLimiter>>,
    Json(credentials): Json<EmailPasswordPair>,
) -> ApiResponse<CreateSessionResponse> {
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
    let user_query = User::by_email(&db, &user_email).await;
    let user_model = match user_query {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Ok(CreateSessionResponse::BadRequest(SessionCreationError {
                message: "Invalid email or password.".to_string(),
            }))
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
            let session_cookie = Cookie::build("session", serialized_session)
                .path("/")
                .max_age(Duration::days(7))
                .finish();

            let updated_cookies = cookies.add(session_cookie);

            Ok(CreateSessionResponse::Created(updated_cookies))
        }
        Ok(false) => Ok(CreateSessionResponse::BadRequest(SessionCreationError {
            message: "Invalid email or password.".to_string(),
        })),
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

async fn get_user_info(session: Session) -> Json<UserInfo> {
    Json(UserInfo {
        user_id: session.user_id(),
    })
}
