use std::convert::{TryFrom, TryInto};

use chrono::{DateTime, NaiveDate, Utc};
use diesel::{
    dsl::AsExprOf, expression::AsExpression, sql_types, Insertable, PgConnection, Queryable,
};
use tracing::trace;
use uuid::Uuid;

use crate::schema::{currency, transaction, transaction_entry};

use super::domain;

sql_function!(
    /// Get an account by owner ID and name, or create it if it doesn't exist.
    ///
    /// This can be helpful when working with accounts by name rather than ID.
    ///
    /// # Arguments
    /// * `owner_id` - The ID of the user who owns the account.
    /// * `account_name` - The name of the account.
    ///
    /// # Returns
    ///
    /// The ID of the account. If the user did not have an account with the
    /// given name, a new one was created.
    fn get_or_create_account(owner_id: sql_types::Uuid, account_name: sql_types::Text) -> sql_types::Uuid
);

#[derive(Clone, Debug, Insertable, Queryable)]
#[table_name = "currency"]
pub struct Currency {
    pub code: String,
    pub symbol: String,
    pub minor_units: i16,
}

impl Currency {
    pub fn find_by_codes(conn: &PgConnection, codes: Vec<String>) -> anyhow::Result<Vec<Self>> {
        use crate::schema::currency::dsl::*;
        use diesel::{dsl::any, prelude::*};

        trace!(?codes, "Finding currencies matching codes.");

        Ok(currency.filter(code.eq(any(codes))).get_results(conn)?)
    }

    pub fn get_by_code(conn: &PgConnection, currency_code: &str) -> anyhow::Result<Option<Self>> {
        use crate::schema::currency::dsl::*;
        use diesel::prelude::*;

        trace!(currency_code, "Querying for currency by code.");

        Ok(currency
            .filter(code.eq(currency_code))
            .first(conn)
            .optional()?)
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

#[derive(Insertable)]
#[table_name = "transaction"]
pub struct NewTransaction {
    user_id: Uuid,
    date: NaiveDate,
    payee: String,
    notes: String,
}

impl From<&domain::transactions::NewTransaction> for NewTransaction {
    fn from(transaction: &domain::transactions::NewTransaction) -> Self {
        Self {
            user_id: transaction.user_id(),
            date: transaction.date(),
            payee: transaction.payee().to_owned(),
            notes: transaction.notes().unwrap_or("").to_owned(),
        }
    }
}

#[derive(Debug, Insertable)]
#[table_name = "transaction_entry"]
pub struct NewTransactionEntry {
    transaction_id: Uuid,
    order: i32,
    #[column_name = "account_id"]
    account: AccountByName,
    currency: String,
    amount: i32,
}

#[derive(Debug, Queryable)]
pub struct Account {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct AccountByName {
    user_id: Uuid,
    name: String,
}

impl AsExpression<diesel::sql_types::Uuid> for AccountByName {
    type Expression = get_or_create_account::HelperType<
        AsExprOf<Uuid, sql_types::Uuid>,
        AsExprOf<String, sql_types::Text>,
    >;

    fn as_expression(self) -> Self::Expression {
        get_or_create_account(self.user_id, self.name)
    }
}

impl<'a> AsExpression<diesel::sql_types::Uuid> for &'a AccountByName {
    type Expression = get_or_create_account::HelperType<
        AsExprOf<Uuid, sql_types::Uuid>,
        AsExprOf<String, sql_types::Text>,
    >;

    fn as_expression(self) -> Self::Expression {
        get_or_create_account(self.user_id, self.name.clone())
    }
}

impl NewTransactionEntry {
    pub fn from_domain_entries(
        transaction_id: Uuid,
        user_id: Uuid,
        entries: &Vec<domain::transactions::TransactionEntry>,
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

#[derive(Debug, Queryable)]
pub struct Transaction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub date: NaiveDate,
    pub payee: String,
    pub notes: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Associations, Debug, Queryable)]
#[belongs_to(Account)]
#[belongs_to(Currency, foreign_key = "currency")]
#[belongs_to(Transaction)]
#[table_name = "transaction_entry"]
pub struct TransactionEntry {
    _id: Uuid,
    transaction_id: Uuid,
    _order: i32,
    account_id: Uuid,
    currency: String,
    amount: i32,
}

/// A full transaction entry contains an entry as well as the associated account
/// and currency.
///
/// To query for it, use the appropriate joins:
///
/// ```no_run
/// # use zeroed_books_api::schema;
/// # use diesel::prelude::*;
///
/// let _join = schema::transaction_entry::table
///     .inner_join(schema::account::table)
///     .inner_join(schema::currency::table);
/// ```
pub type FullTransactionEntry = (TransactionEntry, Account, Currency);

impl TryFrom<&FullTransactionEntry> for domain::transactions::TransactionEntry {
    type Error = anyhow::Error;

    fn try_from(entry: &FullTransactionEntry) -> anyhow::Result<Self> {
        Ok(Self::new(
            entry.1.name.clone(),
            domain::currency::CurrencyAmount::from_minor((&entry.2).try_into()?, entry.0.amount),
        ))
    }
}
