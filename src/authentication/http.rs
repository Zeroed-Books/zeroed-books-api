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
use serde_json::json;
use sqlx::PgPool;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
    client_ip::ClientIp,
    http_err::{ApiError, ApiResponse},
    passwords,
    rate_limit::{RateLimitError, RateLimiter},
    server::AppState,
};

use super::{domain::session::Session, jwt::TokenClaims, models::User};

pub fn routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .with_state(app_state)
        .route("/cookie-sessions", post(create_cookie_session))
        .route("/email-claims", post(claim_email))
        .route("/me", get(get_user_info))
        .route("/new-me", get(get_jwt_info))
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
    match rate_limiter.record_operation(&rate_limit_key, 10) {
        Ok(_) => (),
        Err(result @ RateLimitError::LimitedUntil(_)) => return Err(result.into()),
        Err(err) => {
            error!(error = ?err, "Failed to query rate limiter.");

            return Err(ApiError::InternalServerError);
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

            return Err(ApiError::InternalServerError);
        }
    };

    let parsed_hash = match passwords::Hash::from_hash_str(&user_model.password_hash) {
        Ok(hash) => hash,
        Err(error) => {
            error!(?error, "Invalid password hash received from model.");

            return Err(ApiError::InternalServerError);
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

            Err(ApiError::InternalServerError)
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

async fn get_jwt_info(claims: TokenClaims) -> Json<TokenClaims> {
    Json(claims)
}

enum ClaimEmailResponse {
    Success,
    Error(String),
}

async fn claim_email(
    token: axum_jwks::Token,
    claims: TokenClaims,
    State(db_pool): State<PgPool>,
) -> ApiResponse<ClaimEmailResponse> {
    let profile_url = format!("{}userinfo", claims.iss());
    debug!(%profile_url, "Fetching user profile.");

    let client = reqwest::Client::new();
    let profile: Auth0Profile = client
        .get(profile_url)
        .bearer_auth(token.value())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    info!(?profile, "Retrieved user profile.");

    if !profile.email_verified {
        return Ok(ClaimEmailResponse::Error(
            "Email is not verified.".to_owned(),
        ));
    }

    let user_result = sqlx::query!(
        r#"
        SELECT e.user_id AS id
        FROM email e
        WHERE e.provided_address = $1 AND e.verified_at IS NOT NULL
        "#r,
        &profile.email
    )
    .fetch_optional(&db_pool)
    .await
    .map_err(|error| {
        error!(?error, "Failed to query for user.");

        ApiError::InternalServerError
    })?;

    let user_id = if let Some(user) = user_result {
        user.id
    } else {
        return Ok(ClaimEmailResponse::Error(
            "There are no records corresponding to the authenticated user.".to_owned(),
        ));
    };

    let mut tx = db_pool.begin().await.map_err(|error| {
        error!(?error, "Failed to start transaction.");

        ApiError::InternalServerError
    })?;

    sqlx::query!(
        r#"
        UPDATE account
        SET user_id = $1
        WHERE legacy_user_id = $2
        "#r,
        claims.sub(),
        user_id,
    )
    .execute(&mut tx)
    .await
    .map_err(|error| {
        error!(?error, "Failed to update accounts.");

        ApiError::InternalServerError
    })?;

    sqlx::query!(
        r#"
        UPDATE transaction
        SET user_id = $1
        WHERE legacy_user_id = $2
        "#r,
        claims.sub(),
        user_id,
    )
    .execute(&mut tx)
    .await
    .map_err(|error| {
        error!(?error, "Failed to update transactions.");

        ApiError::InternalServerError
    })?;

    tx.commit().await.map_err(|error| {
        error!(?error, "Failed to commit ownership update.");

        ApiError::InternalServerError
    })?;

    Ok(ClaimEmailResponse::Success)
}

#[derive(Debug, Deserialize)]
struct Auth0Profile {
    email: String,
    email_verified: bool,
}

impl IntoResponse for ClaimEmailResponse {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            Self::Success => (StatusCode::OK, json!({ "message": "Success."})),
            Self::Error(reason) => (StatusCode::BAD_REQUEST, json!({ "message": reason })),
        };

        (status, Json(body)).into_response()
    }
}
