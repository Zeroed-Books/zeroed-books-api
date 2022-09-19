use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::identities::domain;

pub struct PasswordReset {
    pub token: String,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
}

impl From<PasswordReset> for domain::password_resets::PasswordResetTokenData {
    fn from(reset: PasswordReset) -> Self {
        Self {
            user_id: reset.user_id,
            token: reset.token,
            created_at: reset.created_at,
        }
    }
}
