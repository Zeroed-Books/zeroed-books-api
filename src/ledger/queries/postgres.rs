use std::{collections::HashMap, convert::TryInto};

use anyhow::Result;
use async_trait::async_trait;
use sqlx::{Postgres, QueryBuilder, Row};
use tracing::{debug, trace};
use uuid::Uuid;

use crate::{
    database::PostgresConnection,
    ledger::{domain, models},
};

use super::{AccountQueries, CurrencyQueries, TransactionQueries};

/// A struct to provide queries for the Postgres database backing the
/// application.
pub struct PostgresQueries(pub PostgresConnection);

#[derive(sqlx::FromRow)]
struct CurrencyBalance {
    pub currency: String,
    #[sqlx(default)]
    pub amount: i64,
}

#[async_trait]
impl AccountQueries for PostgresQueries {
    async fn get_account_balance(
        &self,
        user_id: &str,
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
        .fetch_all(&*self.0)
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
        .fetch_all(&*self.0)
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
        user_id: &str,
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
            .fetch_all(&*self.0)
            .await?
            .iter()
            .map(|row| row.try_get(0))
            .collect::<Result<Vec<_>, sqlx::Error>>()?)
    }

    async fn list_active_accounts(&self, user_id: &str) -> Result<Vec<String>> {
        let accounts = sqlx::query!(
            r#"
            SELECT DISTINCT a.name
            FROM transaction_entry e
                LEFT JOIN account a ON a.id = e.account_id
                LEFT JOIN transaction t ON t.id = e.transaction_id
            WHERE a.user_id = $1
                AND t.created_at >= now() - INTERVAL '1 year'
            "#,
            user_id
        )
        .fetch_all(&*self.0)
        .await?
        .drain(..)
        .map(|record| record.name)
        .collect::<Vec<_>>();

        Ok(accounts)
    }
}

#[async_trait]
impl CurrencyQueries for PostgresQueries {
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
        .fetch_all(&*self.0)
        .await?;

        let mut currency_map = HashMap::with_capacity(currency_models.len());
        for model in currency_models.iter() {
            currency_map.insert(model.code.clone(), model.try_into()?);
        }

        Ok(currency_map)
    }
}

#[async_trait]
impl TransactionQueries for PostgresQueries {
    async fn get_transaction(
        &self,
        user_id: &str,
        transaction_id: Uuid,
    ) -> Result<Option<domain::transactions::Transaction>> {
        trace!(%user_id, %transaction_id, "Querying for transaction by ID.");

        let transaction_result = sqlx::query_as!(
            models::Transaction,
            r#"
            SELECT id, user_id, date, payee, notes, created_at, updated_at
            FROM transaction
            WHERE user_id = $1 AND id = $2
            "#,
            user_id,
            transaction_id
        )
        .fetch_optional(&*self.0)
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
            WHERE e.transaction_id = $1
            ORDER BY "order"
            "#,
        )
        .bind(transaction_id)
        .fetch_all(&*self.0)
        .await?;

        Ok(Some(transaction.try_into_domain(&entries)?))
    }
}
