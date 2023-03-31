use std::convert::{TryFrom, TryInto};

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{postgres::PgRow, PgPool, Row};
use tracing::trace;
use uuid::Uuid;

use super::domain;

#[derive(Clone, Debug)]
pub struct Currency {
    pub code: String,
    pub symbol: String,
    pub minor_units: i16,
}

impl Currency {
    pub async fn find_by_codes(pool: &PgPool, codes: Vec<String>) -> anyhow::Result<Vec<Self>> {
        trace!(?codes, "Finding currencies matching codes.");

        Ok(sqlx::query_as!(
            Self,
            r#"
            SELECT code, symbol, minor_units
            FROM currency
            WHERE code = ANY($1)
            "#,
            &codes
        )
        .fetch_all(pool)
        .await?)
    }

    pub async fn get_by_code(pool: &PgPool, currency_code: &str) -> anyhow::Result<Option<Self>> {
        trace!(currency_code, "Querying for currency by code.");

        Ok(sqlx::query_as!(
            Self,
            r#"
            SELECT code, symbol, minor_units
            FROM currency
            WHERE code = $1
            "#,
            currency_code
        )
        .fetch_optional(pool)
        .await?)
    }
}

impl From<domain::currency::Currency> for Currency {
    fn from(currency: domain::currency::Currency) -> Self {
        Self {
            code: currency.code().to_owned(),
            symbol: "".to_owned(),
            minor_units: currency.minor_units().into(),
        }
    }
}

impl TryFrom<&Currency> for domain::currency::Currency {
    type Error = anyhow::Error;

    fn try_from(model: &Currency) -> Result<Self, Self::Error> {
        Ok(domain::currency::Currency::new(
            model.code.clone(),
            // We have to use an i16 for the minor units to satisfy the SMALLINT
            // used for the column, but in our domain, we use unsigned integers.
            // We still catch the error here if we somehow end up with a
            // negative number, but we don't care enough about the error to do
            // more specific error handling.
            model.minor_units.try_into()?,
        ))
    }
}

pub struct NewTransaction {
    pub legacy_user_id: Uuid,
    pub date: NaiveDate,
    pub payee: String,
    pub notes: String,
}

impl From<&domain::transactions::NewTransaction> for NewTransaction {
    fn from(transaction: &domain::transactions::NewTransaction) -> Self {
        Self {
            legacy_user_id: transaction.user_id(),
            date: transaction.date(),
            payee: transaction.payee().to_owned(),
            notes: transaction.notes().unwrap_or("").to_owned(),
        }
    }
}

#[derive(Debug)]
pub struct NewTransactionEntry {
    pub transaction_id: Uuid,
    pub order: i32,
    pub account: AccountByName,
    pub currency: String,
    pub amount: i32,
}

#[derive(Clone, Debug)]
pub struct Account {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct AccountByName {
    pub user_id: Uuid,
    pub name: String,
}

impl NewTransactionEntry {
    pub fn from_domain_entries(
        transaction_id: Uuid,
        user_id: Uuid,
        entries: &[domain::transactions::TransactionEntry],
    ) -> anyhow::Result<Vec<Self>> {
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                Ok(Self {
                    transaction_id,
                    order: index.try_into()?,
                    account: AccountByName {
                        user_id,
                        name: entry.account().to_owned(),
                    },
                    currency: entry.amount().currency().code().to_owned(),
                    amount: entry.amount().value(),
                })
            })
            .collect()
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct Transaction {
    pub id: Uuid,
    pub user_id: Option<String>,
    pub legacy_user_id: Option<Uuid>,
    pub date: NaiveDate,
    pub payee: String,
    pub notes: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Transaction {
    pub fn try_into_domain(
        &self,
        entries: &[FullTransactionEntry],
    ) -> anyhow::Result<domain::transactions::Transaction> {
        Ok(domain::transactions::Transaction {
            id: self.id,
            user_id: self.legacy_user_id.unwrap(),
            date: self.date,
            payee: self.payee.clone(),
            notes: self.notes.clone(),
            entries: entries
                .iter()
                .map(|entry| entry.try_into())
                .collect::<anyhow::Result<Vec<domain::transactions::TransactionEntry>>>()?,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
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

/// A full transaction entry contains an entry as well as the associated account
/// and currency.
#[derive(Clone)]
pub struct FullTransactionEntry {
    pub entry: TransactionEntry,
    pub account: Account,
    pub currency: Currency,
}

impl sqlx::FromRow<'_, PgRow> for FullTransactionEntry {
    fn from_row(row: &'_ PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            entry: TransactionEntry {
                id: row.try_get(0)?,
                transaction_id: row.try_get(1)?,
                order: row.try_get(2)?,
                account_id: row.try_get(3)?,
                currency: row.try_get(4)?,
                amount: row.try_get(5)?,
            },
            account: Account {
                id: row.try_get(6)?,
                user_id: row.try_get(7)?,
                name: row.try_get(8)?,
                created_at: row.try_get(9)?,
            },
            currency: Currency {
                code: row.try_get(11)?,
                symbol: row.try_get(12)?,
                minor_units: row.try_get(13)?,
            },
        })
    }
}

impl TryFrom<&FullTransactionEntry> for domain::transactions::TransactionEntry {
    type Error = anyhow::Error;

    fn try_from(entry: &FullTransactionEntry) -> anyhow::Result<Self> {
        Ok(Self::new(
            entry.account.name.clone(),
            domain::currency::CurrencyAmount::from_minor(
                (&entry.currency).try_into()?,
                entry.entry.amount,
            ),
        ))
    }
}
