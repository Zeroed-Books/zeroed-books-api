use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
};

use chrono::{DateTime, NaiveDate, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::ledger::domain;

#[derive(Clone, Debug)]
pub struct Account {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct Currency {
    pub code: String,
    pub symbol: String,
    pub minor_units: i16,
}

/// A transaction that has been persisted in a repository.
#[derive(Debug, sqlx::FromRow)]
pub struct Transaction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub date: NaiveDate,
    pub payee: String,
    pub notes: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct TransactionEntry {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub order: i32,
    pub account_id: Uuid,
    pub currency: String,
    pub amount: i32,
}

#[derive(Clone)]
pub struct FullTransactionEntry {
    pub entry: TransactionEntry,
    pub currency: Currency,
    pub account: Account,
}

pub struct TransactionWithEntries {
    transaction: Transaction,
    entries: Vec<FullTransactionEntry>,
}

impl TryFrom<Currency> for domain::currency::Currency {
    type Error = anyhow::Error;

    fn try_from(model: Currency) -> Result<Self, Self::Error> {
        Ok(domain::currency::Currency::new(
            model.code,
            // We have to use an i16 for the minor units to satisfy the SMALLINT
            // used for the column, but in our domain, we use unsigned integers.
            // We still catch the error here if we somehow end up with a
            // negative number, but we don't care enough about the error to do
            // more specific error handling.
            model.minor_units.try_into()?,
        ))
    }
}

impl TryFrom<FullTransactionEntry> for domain::transactions::TransactionEntry {
    type Error = anyhow::Error;

    fn try_from(model: FullTransactionEntry) -> Result<Self, Self::Error> {
        Ok(Self::new(
            model.account.name,
            domain::currency::CurrencyAmount::from_minor(
                model.currency.try_into()?,
                model.entry.amount,
            ),
        ))
    }
}

impl TransactionWithEntries {
    pub fn zip_with_entries<T, E, C, A>(
        transactions: T,
        entries: E,
        currencies: C,
        accounts: A,
    ) -> Result<Vec<TransactionWithEntries>, TransactionCollationError>
    where
        T: IntoIterator<Item = Transaction>,
        E: IntoIterator<Item = TransactionEntry>,
        C: IntoIterator<Item = Currency>,
        A: IntoIterator<Item = Account>,
    {
        let mut accounts_by_id = HashMap::new();
        for account in accounts {
            accounts_by_id.insert(account.id, account);
        }

        let mut currencies_by_code = HashMap::new();
        for currency in currencies {
            currencies_by_code.insert(currency.code.clone(), currency);
        }

        let mut entries_by_transaction_id: HashMap<Uuid, Vec<FullTransactionEntry>> =
            HashMap::new();
        for entry in entries {
            let transaction_id = entry.transaction_id;
            let account = accounts_by_id.get(&entry.account_id).ok_or(
                TransactionCollationError::UnmatchedAccount(entry.account_id),
            )?;
            let currency = currencies_by_code.get(&entry.currency).ok_or_else(|| {
                TransactionCollationError::UnmatchedCurrency(entry.currency.clone())
            })?;

            let full_entry = FullTransactionEntry {
                entry,
                account: account.clone(),
                currency: currency.clone(),
            };

            entries_by_transaction_id
                .entry(transaction_id)
                .and_modify(|transaction_entries| transaction_entries.push(full_entry.clone()))
                .or_insert_with(|| vec![full_entry]);
        }

        Ok(transactions
            .into_iter()
            .map(|transaction| {
                let entries = entries_by_transaction_id
                    .get(&transaction.id)
                    .unwrap_or(&vec![])
                    .to_vec();

                TransactionWithEntries {
                    transaction,
                    entries,
                }
            })
            .collect::<Vec<_>>())
    }
}

impl TryFrom<TransactionWithEntries> for domain::transactions::Transaction {
    type Error = anyhow::Error;

    fn try_from(mut model: TransactionWithEntries) -> Result<Self, Self::Error> {
        let entries: Vec<domain::transactions::TransactionEntry> = model
            .entries
            .drain(..)
            .map(FullTransactionEntry::try_into)
            .collect::<anyhow::Result<_>>()?;

        Ok(Self {
            id: model.transaction.id,
            user_id: model.transaction.user_id,
            date: model.transaction.date,
            payee: model.transaction.payee,
            notes: model.transaction.notes,
            entries,
            created_at: model.transaction.created_at,
            updated_at: model.transaction.updated_at,
        })
    }
}

#[derive(Debug, Error)]
pub enum TransactionCollationError {
    #[error("the transaction references the account ID {0:?} which is not present")]
    UnmatchedAccount(Uuid),
    #[error("the transaction references the currency {0:?} which is not present")]
    UnmatchedCurrency(String),
}
