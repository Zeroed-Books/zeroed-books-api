use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

use crate::identities::{
    domain::password_resets::PasswordResetTokenData, models::password_resets::PasswordReset,
};

use super::{PasswordResetError, PasswordResetQueries};

pub struct PostgresQueries<'a>(pub &'a PgPool);

impl From<sqlx::Error> for PasswordResetError {
    fn from(error: sqlx::Error) -> Self {
        Self::Unknown(error.into())
    }
}

#[async_trait]
impl<'a> PasswordResetQueries for PostgresQueries<'a> {
    async fn get_password_reset(
        &self,
        provided_token: String,
    ) -> Result<PasswordResetTokenData, PasswordResetError> {
        let result = sqlx::query_as!(
            PasswordReset,
            r#"
            SELECT * FROM password_resets
            WHERE token = $1
            "#,
            provided_token
        )
        .fetch_optional(self.0)
        .await?;

        match result {
            Some(reset) => Ok(reset.into()),
            None => Err(PasswordResetError::NotFound),
        }
    }
}
