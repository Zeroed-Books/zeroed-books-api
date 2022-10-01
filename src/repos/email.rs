use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use crate::{database::PostgresConnection, identities::models::email::NewEmailVerification};

#[derive(Debug, Error)]
pub enum EmailVerificationError {
    #[error("invalid verification token")]
    InvalidToken,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type DynEmailRepo = Arc<dyn EmailRepo + Send + Sync>;

#[async_trait]
pub trait EmailRepo {
    /// Delete a specific email verification.
    async fn delete_verification_by_token(&self, token: &str) -> anyhow::Result<()>;

    /// Insert a new email verification object.
    async fn insert_verification(
        &self,
        email_verification: &NewEmailVerification,
    ) -> anyhow::Result<()>;

    /// Mark a specific email as verified using a verification token.
    async fn mark_email_as_verified(&self, token: &str) -> Result<String, EmailVerificationError>;
}

#[async_trait]
impl EmailRepo for PostgresConnection {
    async fn delete_verification_by_token(&self, token: &str) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM email_verification
            WHERE token = $1
            "#,
            token
        )
        .execute(&**self)
        .await?;

        Ok(())
    }

    async fn insert_verification(
        &self,
        email_verification: &NewEmailVerification,
    ) -> anyhow::Result<()> {
        email_verification.save(self).await
    }

    async fn mark_email_as_verified(&self, token: &str) -> Result<String, EmailVerificationError> {
        let verified_address = sqlx::query!(
            r#"
            WITH pending_verification_emails AS (
                SELECT email_id
                FROM email_verification
                WHERE token = $1 AND created_at > (now() - INTERVAL '1 DAY')
            )
            UPDATE email
            SET verified_at = now()
            WHERE id = ANY(SELECT * FROM pending_verification_emails)
            RETURNING provided_address
            "#,
            token,
        )
        .fetch_optional(&**self)
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
                .execute(&**self)
                .await?;

                Ok(address)
            }
            None => Err(EmailVerificationError::InvalidToken),
        }
    }
}

impl From<sqlx::Error> for EmailVerificationError {
    fn from(error: sqlx::Error) -> Self {
        Self::Other(error.into())
    }
}
