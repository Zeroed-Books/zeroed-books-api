pub mod postgres;

use uuid::Uuid;

use super::domain;

#[async_trait]
pub trait Queries {
    async fn latest_transactions(
        &self,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<domain::transactions::Transaction>>;
}
