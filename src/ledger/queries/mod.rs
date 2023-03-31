//! Queries for ledger information.
//!
//! Queries fetch information from whatever storage is backing the application.
//! They never modify data.

pub mod postgres;

use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use super::domain::{self, currency::CurrencyAmount};

/// Queries for account information.
#[async_trait]
pub trait AccountQueries {
    /// Get the balance for an account.
    ///
    /// # Arguments
    ///
    /// * `user_id` - ID of the user who owns the account.
    /// * `account_name` - The name of the account.
    ///
    /// # Returns
    ///
    /// A [`Vec`] of balances for each currency used in transactions attached to
    /// the specified account.
    async fn get_account_balance(
        &self,
        user_id: &str,
        account_name: String,
    ) -> Result<Vec<CurrencyAmount>>;

    /// List accounts by popularity.
    ///
    /// # Arguments
    ///
    /// * `user_id` - The ID of the user to list accounts for.
    /// * `search_string` - An optional search string used to match account
    ///   names. If given, only accounts containing the given search string will
    ///   be matched.
    ///
    /// # Returns
    ///
    /// A list of account names ranked by the number of transaction entries
    /// associated with them.
    async fn list_accounts_by_popularity(
        &self,
        user_id: &str,
        search_string: Option<String>,
    ) -> Result<Vec<String>>;
}

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
        user_id: &str,
        transaction_id: Uuid,
    ) -> anyhow::Result<Option<domain::transactions::Transaction>>;
}

#[derive(Default)]
pub struct TransactionQuery {
    pub user_id: String,
    pub after: Option<domain::transactions::TransactionCursor>,
    pub account: Option<String>,
}

pub struct TransactionCollection {
    pub next: Option<domain::transactions::TransactionCursor>,
    pub items: Vec<domain::transactions::Transaction>,
}
