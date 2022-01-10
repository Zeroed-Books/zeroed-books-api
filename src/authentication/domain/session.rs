use anyhow::Result;
use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
    Request,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
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

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Session {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        if let Some(session_cookie) = request.cookies().get_private("session") {
            match serde_json::from_str::<Session>(session_cookie.value()) {
                Ok(session) => {
                    debug!(user_id = %session.user_id(), session_id = %session.id(), "Parsed cookie session.");

                    Outcome::Success(session)
                }
                Err(error) => {
                    warn!(
                        ?error,
                        value = session_cookie.value(),
                        "Received malformed session value."
                    );

                    Outcome::Failure((Status::Unauthorized, ()))
                }
            }
        } else {
            Outcome::Failure((Status::Unauthorized, ()))
        }
    }
}
