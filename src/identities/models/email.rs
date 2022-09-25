use sqlx::{Executor, PgPool, Postgres};
use thiserror::Error;
use uuid::Uuid;

use crate::identities::domain::email::{Email, EmailVerification};

#[derive(Clone, Debug)]
pub struct NewEmail {
    id: Uuid,
    user_id: Uuid,
    provided_address: String,
    normalized_address: String,
}

impl NewEmail {
    /// Create an email that can be inserted into the database belonging to a
    /// specific user.
    ///
    /// # Arguments
    ///
    /// * `user_id` - The ID of the user who owns the email address.
    /// * `email` - The email address to persist.
    pub fn for_user(user_id: Uuid, email: &Email) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            provided_address: email.address().to_owned(),
            normalized_address: email.address().to_owned(),
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub async fn save<'c, E>(&self, executor: E) -> Result<(), EmailPersistanceError>
    where
        E: Executor<'c, Database = Postgres>,
    {
        let result = sqlx::query!(
            r#"
            INSERT INTO email (id, user_id, provided_address, normalized_address)
            VALUES ($1, $2, $3, $4)
            "#,
            self.id,
            self.user_id,
            self.provided_address,
            self.normalized_address
        )
        .execute(executor)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(db_err)) if db_err.code().unwrap_or_default() == "23505" => {
                Err(EmailPersistanceError::DuplicateEmail(self.clone()))
            }
            Err(err) => Err(err.into()),
        }
    }
}

#[derive(Debug, Error)]
pub enum EmailPersistanceError {
    #[error("database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("duplicate email address: {0:?}")]
    DuplicateEmail(NewEmail),
}

#[derive(Debug)]
pub struct NewEmailVerification {
    token: String,
    email_id: Uuid,
}

impl NewEmailVerification {
    pub fn new(email_id: Uuid, verification: &EmailVerification) -> Self {
        Self {
            token: verification.token().to_string(),
            email_id,
        }
    }

    pub async fn save(&self, pool: &PgPool) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
                INSERT INTO email_verification (token, email_id)
                VALUES ($1, $2)
                "#,
            self.token,
            self.email_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}
