use std::{collections::HashMap, convert::TryInto};

use anyhow::Result;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Row};
use tracing::{debug, trace};
use uuid::Uuid;

use crate::ledger::{
    domain::{self, transactions::TransactionCursor},
    models,
};

use super::{
    AccountQueries, CurrencyQueries, TransactionCollection, TransactionQueries, TransactionQuery,
};

/// A struct to provide queries for the Postgres database backing the
/// application.
pub struct PostgresQueries<'a>(pub &'a PgPool);

#[derive(sqlx::FromRow)]
struct CurrencyBalance {
    pub currency: String,
    #[sqlx(default)]
    pub amount: i64,
}

#[async_trait]
impl<'a> AccountQueries for PostgresQueries<'a> {
    async fn get_account_balance(
        &self,
        user_id: Uuid,
        account_name: String,
    ) -> Result<Vec<domain::currency::CurrencyAmount>> {
        trace!(account = %account_name, "Fetching account balance.");

        let amounts = sqlx::query_as!(
            CurrencyBalance,
            r#"
            SELECT e."currency", COALESCE(SUM(e."amount"), 0) AS "amount!"
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
            user_id,
            &account_name
        )
        .fetch_all(self.0)
        .await?;

        let currency_codes = amounts
            .iter()
            .map(|balance| balance.currency.clone())
            .collect::<Vec<_>>();

        let mut currencies = sqlx::query_as!(
            models::Currency,
            r#"
            SELECT * FROM currency
            WHERE code = ANY($1)
            ORDER BY code
            "#,
            &currency_codes
        )
        .fetch_all(self.0)
        .await?;

        let balances = currencies.drain(..).zip(amounts).collect::<Vec<_>>();

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
        let mut query_builder: QueryBuilder<'_, Postgres> = QueryBuilder::new(
            r#"
            SELECT a.name
            FROM transaction_entry e
            LEFT JOIN account a ON e.account_id = a.id
            WHERE a.user_id =
            "#,
        );
        query_builder.push_bind(user_id);

        if let Some(search_str) = search {
            query_builder
                .push("AND a.name ILIKE '%' || ")
                .push_bind(search_str)
                .push(" || '%'");
        }

        query_builder.push(
            r#"
            GROUP BY a.id
            ORDER BY COUNT(e.*) DESC
            LIMIT 10
            "#,
        );

        Ok(query_builder
            .build()
            .fetch_all(self.0)
            .await?
            .iter()
            .map(|row| row.try_get(0))
            .collect::<Result<Vec<_>, sqlx::Error>>()?)
    }
}

#[async_trait]
impl<'a> CurrencyQueries for PostgresQueries<'a> {
    async fn get_currencies_by_code(
        &self,
        currency_codes: Vec<String>,
    ) -> Result<HashMap<String, domain::currency::Currency>> {
        let currency_models = sqlx::query_as!(
            models::Currency,
            r#"
            SELECT * FROM currency
            WHERE code = ANY($1)
            "#,
            &currency_codes
        )
        .fetch_all(self.0)
        .await?;

        let mut currency_map = HashMap::with_capacity(currency_models.len());
        for model in currency_models.iter() {
            currency_map.insert(model.code.clone(), model.try_into()?);
        }

        Ok(currency_map)
    }
}

const TRANSACTION_PAGE_SIZE: u8 = 50;

#[async_trait]
impl<'a> TransactionQueries for PostgresQueries<'a> {
    async fn get_transaction(
        &self,
        user_id: Uuid,
        transaction_id: Uuid,
    ) -> Result<Option<domain::transactions::Transaction>> {
        trace!(%user_id, %transaction_id, "Querying for transaction by ID.");

        let transaction_result = sqlx::query_as!(
            models::Transaction,
            r#"
            SELECT * FROM transaction
            WHERE user_id = $1 AND id = $2
            "#,
            user_id,
            transaction_id
        )
        .fetch_optional(self.0)
        .await?;

        let transaction = match transaction_result {
            Some(t) => t,
            None => {
                debug!(%user_id, %transaction_id, "Transaction does not exist.");

                return Ok(None);
            }
        };

        let entries = sqlx::query_as::<_, models::FullTransactionEntry>(
            r#"
            SELECT e.*, a.*, c.*
            FROM transaction_entry e
                LEFT JOIN account a ON e.account_id = a.id
                LEFT JOIN currency c ON e.currency = c.code
            ORDER BY "order"
            "#,
        )
        .fetch_all(self.0)
        .await?;

        Ok(Some(transaction.try_into_domain(&entries)?))
    }

    async fn list_transactions(&self, query: TransactionQuery) -> Result<TransactionCollection> {
        let mut query_builder: QueryBuilder<'_, Postgres> = QueryBuilder::new("");

        if let Some(account) = query.account.as_ref() {
            query_builder
                .push(
                    r#"
                    WITH account_transaction_ids AS (
                        SELECT DISTINCT t.id
                        FROM transaction_entry e
                            LEFT JOIN transaction t ON e.transaction_id = t.id
                            LEFT JOIN account a ON e.account_id = a.id
                        WHERE a.name = "#,
                )
                .push_bind(account)
                .push(" OR a.name LIKE ")
                .push_bind(account)
                .push(" || ':%' )");
        }

        query_builder
            .push(
                r#"
                SELECT t.*
                FROM transaction t
                WHERE t.user_id = "#,
            )
            .push_bind(query.user_id);

        if query.account.is_some() {
            query_builder.push(" AND t.id = ANY(SELECT * FROM account_transaction_ids)");
        }

        if let Some(cursor) = query.after {
            query_builder
                .push(" AND (t.date < ")
                .push_bind(cursor.after_date)
                .push(" OR (t.date = ")
                .push_bind(cursor.after_date)
                .push(" AND t.created_at < ")
                .push_bind(cursor.after_created_at)
                .push("))");
        }

        query_builder
            .push(" ORDER BY t.date DESC, t.created_at DESC LIMIT ")
            .push_bind(i16::from(TRANSACTION_PAGE_SIZE));

        let mut transactions_data: Vec<models::Transaction> = query_builder
            .build()
            .fetch_all(self.0)
            .await?
            .iter()
            .map(|row| models::Transaction::from_row(row))
            .collect::<Result<Vec<_>, sqlx::Error>>()?;

        // To figure out if there is a next page, we query one more element than
        // the maximum page size. If it exists, we remove it from the page, but
        // remember that there are more elements.
        let has_next_page = transactions_data.len() > usize::from(TRANSACTION_PAGE_SIZE);
        if has_next_page {
            transactions_data.pop();
        }

        let entries = sqlx::query_as::<_, models::FullTransactionEntry>(
            r#"
            SELECT e.*, a.*, c.*
            FROM transaction_entry e
                LEFT JOIN account a ON e.account_id = a.id
                LEFT JOIN currency c ON e.currency = c.code
                LEFT JOIN transaction t ON e.transaction_id = t.id
            ORDER BY t.id, e."order"
            "#,
        )
        .fetch_all(self.0)
        .await?;

        let mut entries_by_transaction: HashMap<Uuid, Vec<models::FullTransactionEntry>> =
            HashMap::new();
        for entry in entries {
            entries_by_transaction
                .entry(entry.entry.transaction_id)
                .and_modify(|entries| entries.push(entry.clone()))
                .or_insert(vec![entry]);
        }

        let transactions = transactions_data
            .iter()
            .map(|transaction| {
                transaction.try_into_domain(
                    entries_by_transaction
                        .get(&transaction.id)
                        .unwrap_or(&vec![]),
                )
            })
            .collect::<Result<Vec<domain::transactions::Transaction>>>()?;

        let cursor = if has_next_page {
            let last_transaction = &transactions[transactions.len() - 1];

            Some(TransactionCursor {
                after_date: last_transaction.date,
                after_created_at: last_transaction.created_at,
            })
        } else {
            None
        };

        Ok(TransactionCollection {
            next: cursor,
            items: transactions,
        })
    }
}
