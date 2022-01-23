use uuid::Uuid;

use super::domain::transactions::{NewTransaction, Transaction};

pub mod postgres;

#[async_trait]
pub trait Commands {
    /// Delete a transaction.
    ///
    /// # Arguments
    ///
    /// * `user_id` - The ID of the transaction's owner.
    /// * `transaction_id` - The ID of the transaction to delete.
    ///
    /// # Returns
    ///
    /// A [`Result`][anyhow::Result] containing either an empty success value,
    /// or an error that occurred. Attempting to delete a transaction that does
    /// not exist is not an error.
    async fn delete_transaction(&self, user_id: Uuid, transaction_id: Uuid) -> anyhow::Result<()>;

    /// Persist a new transaction.
    ///
    /// # Arguments
    /// * `transaction` - The transaction to persist.
    ///
    /// # Returns
    ///
    /// A result containing either an error or the information about the
    /// transaction that was persisted.
    async fn persist_transaction(&self, transaction: NewTransaction)
        -> anyhow::Result<Transaction>;
}
