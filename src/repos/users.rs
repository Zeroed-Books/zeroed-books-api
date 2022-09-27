use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use crate::{
    database::PostgresConnection,
    identities::models::email::{EmailPersistanceError, NewEmail},
    models::NewUserModel,
};

#[derive(Debug, Error)]
pub enum UserPersistenceError {
    #[error("duplicate email address: {0:?}")]
    DuplicateEmail(NewEmail),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type DynUserRepo = Arc<dyn UserRepo + Send + Sync>;

#[async_trait]
pub trait UserRepo {
    async fn persist_new_user(
        &self,
        user: &NewUserModel,
        email: &NewEmail,
    ) -> Result<(), UserPersistenceError>;
}

#[async_trait]
impl UserRepo for PostgresConnection {
    async fn persist_new_user(
        &self,
        user: &NewUserModel,
        email: &NewEmail,
    ) -> Result<(), UserPersistenceError> {
        let mut tx = self.begin().await.map_err(anyhow::Error::from)?;

        sqlx::query!(
            r#"
            INSERT INTO "user" (id, password)
            VALUES ($1, $2)
            "#,
            user.id,
            user.password_hash
        )
        .execute(&mut tx)
        .await
        .map_err(anyhow::Error::from)?;

        email.save(&mut tx).await.map_err(|error| match error {
            EmailPersistanceError::DuplicateEmail(email) => {
                UserPersistenceError::DuplicateEmail(email)
            }
            other => UserPersistenceError::Other(other.into()),
        })?;

        tx.commit().await.map_err(anyhow::Error::from)?;

        Ok(())
    }
}
