use std::sync::Arc;

use async_trait::async_trait;

use crate::{database::PostgresConnection, identities::models::email::NewEmailVerification};

pub type DynEmailRepo = Arc<dyn EmailRepo + Send + Sync>;

#[async_trait]
pub trait EmailRepo {
    async fn insert_verification(
        &self,
        email_verification: &NewEmailVerification,
    ) -> anyhow::Result<()>;
}

#[async_trait]
impl EmailRepo for PostgresConnection {
    async fn insert_verification(
        &self,
        email_verification: &NewEmailVerification,
    ) -> anyhow::Result<()> {
        email_verification.save(self).await
    }
}
