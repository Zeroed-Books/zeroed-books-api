pub mod postgres;

use uuid::Uuid;

use super::domain;

#[async_trait]
pub trait Queries {
    /// Get a single transaction by its ID.
    ///
    /// # Arguments
    ///
    /// * `user_id` - The ID of the transaction's owner.
    /// * `transaction_id` - The ID of the transaction.
    ///
    /// # Returns
    ///
    /// A [`Result`][anyhow::Result] containing the transaction if it was found.
    async fn get_transaction(
        &self,
        user_id: Uuid,
        transaction_id: Uuid,
    ) -> anyhow::Result<Option<domain::transactions::Transaction>>;

    async fn latest_transactions(
        &self,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<domain::transactions::Transaction>>;
}
