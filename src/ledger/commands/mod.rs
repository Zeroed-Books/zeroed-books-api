use super::domain::transactions::{NewTransaction, Transaction};

pub mod postgres;

#[async_trait]
pub trait Commands {
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
