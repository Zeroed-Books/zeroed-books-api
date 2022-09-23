use anyhow::Result;
use async_trait::async_trait;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
};
use axum_extra::extract::{cookie::Key, PrivateCookieJar};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
pub struct Session {
    id: Uuid,
    user_id: Uuid,
}

impl Session {
    /// Create a new session for a specific user.
    ///
    /// # Example
    ///
    /// ```
    /// # use uuid::Uuid;
    /// # use zeroed_books_api::authentication::domain::session::Session;
    ///
    /// let user_id = Uuid::new_v4();
    /// let session = Session::new_for_user(user_id);
    ///
    /// assert_eq!(user_id, session.user_id());
    /// ```
    pub fn new_for_user(user_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
        }
    }

    pub fn serialized(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn user_id(&self) -> Uuid {
        self.user_id
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Session
where
    S: Send + Sync,
    Key: FromRef<S>,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let jar: PrivateCookieJar<Key> = PrivateCookieJar::from_request_parts(parts, state)
            .await
            .map_err(|error| {
            error!(?error, "Failed to create private cookie jar.");

            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        if let Some(session_cookie) = jar.get("session") {
            match serde_json::from_str::<Session>(session_cookie.value()) {
                Ok(session) => {
                    debug!(user_id = %session.user_id(), session_id = %session.id(), "Parsed cookie session.");

                    return Ok(session);
                }
                Err(error) => {
                    warn!(
                        ?error,
                        value = session_cookie.value(),
                        "Received malformed session value."
                    );
                }
            }
        } else {
            debug!("No session cookie.");
        }

        Err(StatusCode::UNAUTHORIZED)
    }
}

pub struct ExtractSession(pub Session);

#[async_trait]
impl<S> FromRequestParts<S> for ExtractSession
where
    S: Send + Sync,
    Key: FromRef<S>,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let jar: PrivateCookieJar<Key> = PrivateCookieJar::from_request_parts(parts, state)
            .await
            .map_err(|error| {
            error!(?error, "Failed to create private cookie jar.");

            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        if let Some(session_cookie) = jar.get("session") {
            match serde_json::from_str::<Session>(session_cookie.value()) {
                Ok(session) => {
                    debug!(user_id = %session.user_id(), session_id = %session.id(), "Parsed cookie session.");

                    return Ok(ExtractSession(session));
                }
                Err(error) => {
                    warn!(
                        ?error,
                        value = session_cookie.value(),
                        "Received malformed session value."
                    );
                }
            }
        } else {
            debug!("No session cookie.");
        }

        Err(StatusCode::UNAUTHORIZED)
    }
}
