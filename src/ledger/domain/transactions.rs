mod new_transaction;
mod new_transaction_data;
mod new_transaction_entry_data;

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

use super::currency::{Currency, CurrencyAmount};

pub use new_transaction::{NewTransaction, NewTransactionEntry};
pub use new_transaction_data::NewTransactionData;

#[derive(Debug, Eq, PartialEq)]
pub enum NewTransactionError {
    /// The transaction has no entries.
    NoEntries,

    /// The entries in the transaction are not balanced, ie they do not sum to
    /// zero. The value is a mapping of currencies to balances.
    Unbalanced(HashMap<Currency, i32>),
}

pub struct Transaction {
    pub id: Uuid,
    pub user_id: String,
    pub date: NaiveDate,
    pub payee: String,
    pub notes: String,
    pub entries: Vec<TransactionEntry>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionEntry {
    account: String,
    amount: CurrencyAmount,
}

impl TransactionEntry {
    pub fn new(account: String, amount: CurrencyAmount) -> Self {
        Self { account, amount }
    }

    pub fn account(&self) -> &str {
        &self.account
    }

    pub fn amount(&self) -> &CurrencyAmount {
        &self.amount
    }
}

/// A cursor into a collection of transactions. Since transactions are always
/// ordered by descending date and creation time, we can use those fields to
/// mark arbitrary locations in the collection.
pub struct TransactionCursor {
    pub after_date: NaiveDate,
    pub after_created_at: DateTime<Utc>,
}
