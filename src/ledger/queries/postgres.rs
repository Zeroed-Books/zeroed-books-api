use std::{collections::HashMap, convert::TryInto};

use anyhow::Result;
use diesel::{sql_query, sql_types};
use tracing::{debug, trace};
use uuid::Uuid;

use crate::{
    ledger::{
        domain::{self, transactions::TransactionCursor},
        models,
    },
    schema, PostgresConn,
};

use super::{
    AccountQueries, CurrencyQueries, TransactionCollection, TransactionQueries, TransactionQuery,
};

/// A struct to provide queries for the Postgres database backing the
/// application.
pub struct PostgresQueries<'a>(pub &'a PostgresConn);

#[derive(QueryableByName)]
struct CurrencyBalance {
    #[sql_type = "diesel::sql_types::Text"]
    pub currency: String,
    #[sql_type = "diesel::sql_types::BigInt"]
    pub amount: i64,
}

#[async_trait]
impl<'a> AccountQueries for PostgresQueries<'a> {
    async fn get_account_balance(
        &self,
        user_id: Uuid,
        account_name: String,
    ) -> Result<Vec<domain::currency::CurrencyAmount>> {
        let balances = self
            .0
            .run::<_, Result<_>>(move |conn| {
                use diesel::prelude::*;
                use diesel::sql_types;

                let amounts_query = sql_query(
                    r#"
                        SELECT e."currency", COALESCE(SUM(e."amount"), 0) AS amount
                            FROM transaction_entry e
                                JOIN account a ON a.id = e.account_id
                                JOIN transaction t ON t.id = e.transaction_id
                        WHERE
                            t.user_id = $1
                            AND
                                (a.name = $2 OR a.name LIKE $2 || ':%')
                        GROUP BY e.currency
                        ORDER BY e.currency
                    "#,
                )
                .bind::<sql_types::Uuid, _>(user_id)
                .bind::<sql_types::Text, _>(&account_name);

                trace!(
                    account = %account_name,
                    query = %diesel::debug_query(&amounts_query),
                    "Fetching account balance."
                );

                let amounts = amounts_query.load::<CurrencyBalance>(conn)?;

                let currency_codes = amounts
                    .iter()
                    .map(|balance| &balance.currency)
                    .collect::<Vec<_>>();

                let mut currencies = {
                    use schema::currency::dsl::*;

                    currency
                        .filter(code.eq_any(currency_codes))
                        .order_by(code)
                        .load::<models::Currency>(conn)?
                };

                Ok(currencies.drain(..).zip(amounts).collect::<Vec<_>>())
            })
            .await?;

        Ok(balances
            .iter()
            .map(|(currency, amount)| {
                Ok(domain::currency::CurrencyAmount::from_minor(
                    domain::currency::Currency::try_from(currency)?,
                    amount.amount.try_into().unwrap(),
                ))
            })
            .collect::<Result<_>>()?)
    }

    async fn list_accounts_by_popularity(
        &self,
        user_id: Uuid,
        search: Option<String>,
    ) -> Result<Vec<String>> {
        Ok(self
            .0
            .run::<_, Result<_>>(move |conn| {
                use diesel::dsl::sql;
                use diesel::prelude::*;

                let mut query = schema::transaction_entry::table
                    .inner_join(schema::account::table)
                    .select(schema::account::name)
                    .filter(schema::account::user_id.eq(user_id))
                    .group_by(schema::account::id)
                    .order(sql::<sql_types::Integer>("COUNT(transaction_entry.*)").desc())
                    .limit(10)
                    .into_boxed();

                if let Some(search_str) = search {
                    query = query.filter(
                        schema::account::name.ilike(
                            sql("'%' || ")
                                .bind::<sql_types::Text, _>(search_str)
                                .sql(" || '%'"),
                        ),
                    )
                }

                trace!(query = %diesel::debug_query(&query), "Listing accounts by popularity.");

                Ok(query.load::<String>(conn)?)
            })
            .await?)
    }
}

#[async_trait]
impl<'a> CurrencyQueries for PostgresQueries<'a> {
    async fn get_currencies_by_code(
        &self,
        currency_codes: Vec<String>,
    ) -> Result<HashMap<String, domain::currency::Currency>> {
        use diesel::prelude::*;
        use schema::currency::dsl::*;

        let currency_models = self
            .0
            .run::<_, anyhow::Result<_>>(move |conn| {
                Ok(currency
                    .filter(code.eq_any(currency_codes))
                    .get_results::<models::Currency>(conn)?)
            })
            .await?;

        let mut currency_map = HashMap::with_capacity(currency_models.len());
        for model in currency_models.iter() {
            currency_map.insert(model.code.clone(), model.try_into()?);
        }

        Ok(currency_map)
    }
}

type TransactionWithEntries = (models::Transaction, Vec<models::FullTransactionEntry>);

// Why is this a u8? It has to be convertable to an i64 for limiting the query
// size, and convertable to usize for comparing to the length of a vector. This
// is the type that fits.
const TRANSACTION_PAGE_SIZE: u8 = 50;

#[async_trait]
impl<'a> TransactionQueries for PostgresQueries<'a> {
    async fn get_transaction(
        &self,
        user_id: Uuid,
        transaction_id: Uuid,
    ) -> Result<Option<domain::transactions::Transaction>> {
        use diesel::prelude::*;

        trace!(%user_id, %transaction_id, "Querying for transaction by ID.");

        let transaction_data: Option<TransactionWithEntries> = self
            .0
            .run::<_, anyhow::Result<_>>(move |conn| {
                let transaction_query = schema::transaction::table
                    .filter(
                        schema::transaction::user_id
                            .eq(user_id)
                            .and(schema::transaction::id.eq(transaction_id)),
                    )
                    .get_result::<models::Transaction>(conn)
                    .optional()?;

                let transaction = match transaction_query {
                    Some(t) => t,
                    None => {
                        debug!(%user_id, %transaction_id, "Transaction does not exist.");

                        return Ok(None);
                    }
                };

                let entries = models::TransactionEntry::belonging_to(&transaction)
                    .inner_join(schema::account::table)
                    .inner_join(schema::currency::table)
                    .order(schema::transaction_entry::order)
                    .load::<models::FullTransactionEntry>(conn)?;

                Ok(Some((transaction, entries)))
            })
            .await?;

        match transaction_data {
            Some((transaction, entries)) => {
                debug!(user_id = %transaction.user_id, transaction_id = %transaction.id, "Found transaction by ID.");

                Ok(Some(transaction.try_into_domain(&entries)?))
            }
            None => Ok(None),
        }
    }

    async fn list_transactions(&self, query: TransactionQuery) -> Result<TransactionCollection> {
        let (transaction_data, has_next_page) = self
            .0
            .run::<_, Result<_>>(move |conn| {
                use diesel::prelude::*;
                let filter = {
                    use schema::transaction::dsl::*;

                    let mut matching_transactions = transaction
                        .filter(user_id.eq(query.user_id))
                        .order((date.desc(), created_at.desc()))
                        .into_boxed();

                    if let Some(ref account) = query.account {
                        use diesel::dsl::sql;
                        use diesel::sql_types::Text;

                        let account_transactions = schema::transaction_entry::table
                            .left_join(transaction)
                            .left_join(schema::account::table)
                            .filter(
                                schema::account::name.eq(account).or(schema::account::name
                                    .like(sql("").bind::<Text, _>(account).sql(" || ':%'"))),
                            )
                            .distinct_on(id)
                            .select(id);

                        matching_transactions =
                            matching_transactions.filter(id.eq_any(account_transactions));
                    }

                    if let Some(cursor) = query.after {
                        matching_transactions = matching_transactions.filter(
                            date.lt(cursor.after_date).or(date
                                .eq(cursor.after_date)
                                .and(created_at.lt(cursor.after_created_at))),
                        );
                    }

                    matching_transactions.limit(i64::from(TRANSACTION_PAGE_SIZE) + 1)
                };

                trace!(
                    query = %diesel::debug_query(&filter),
                    "Listing transactions."
                );

                let mut transactions = filter.get_results::<models::Transaction>(conn)?;

                // To figure out if there is a next page, we query one more
                // element than the maximum page size. If it exists, we remove
                // it from the page, but remember that there are more elements.
                let has_next_page = transactions.len() == usize::from(TRANSACTION_PAGE_SIZE) + 1;
                if has_next_page {
                    transactions.pop();
                }

                let entries = models::TransactionEntry::belonging_to(&transactions)
                    .inner_join(schema::account::table)
                    .inner_join(schema::currency::table)
                    .order(schema::transaction_entry::order)
                    .load::<models::FullTransactionEntry>(conn)?
                    .grouped_by(&transactions);

                Ok((
                    transactions
                        .drain(..)
                        .zip(entries)
                        .collect::<Vec<TransactionWithEntries>>(),
                    has_next_page,
                ))
            })
            .await?;

        let transactions = transaction_data
            .iter()
            .map(|(transaction, entries)| transaction.try_into_domain(entries))
            .collect::<Result<Vec<domain::transactions::Transaction>>>()?;

        let cursor = match has_next_page {
            true => {
                let last_transaction = &transactions[transactions.len() - 1];

                Some(TransactionCursor {
                    after_date: last_transaction.date,
                    after_created_at: last_transaction.created_at,
                })
            }
            false => None,
        };

        Ok(TransactionCollection {
            next: cursor,
            items: transactions,
        })
    }
}
