pub mod postgres;

use std::collections::HashMap;

use uuid::Uuid;

use super::domain;

#[async_trait]
pub trait CurrencyQueries {
    /// Get a mapping of currency codes to currency objects.
    ///
    /// # Arguments
    ///
    /// * `currency_codes` - The codes of the currencies to retrieve.
    async fn get_currencies_by_code(
        &self,
        currency_codes: Vec<String>,
    ) -> anyhow::Result<HashMap<String, domain::currency::Currency>>;
}

#[async_trait]
pub trait TransactionQueries {
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