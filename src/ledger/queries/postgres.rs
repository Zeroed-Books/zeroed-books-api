use std::{collections::HashMap, convert::TryInto};

use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::{Postgres, QueryBuilder, Row};
use tracing::{debug, trace};
use uuid::Uuid;

use crate::{
    database::PostgresConnection,
    ledger::{
        domain::{
            self,
            currency::{Currency, CurrencyAmount},
            reports::InstantBalances,
        },
        models,
    },
};

use super::{AccountQueries, CurrencyQueries, ReportInterval, TransactionQueries};

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

    async fn get_monthly_balance(
        &self,
        user_id: &str,
        account_name: &str,
    ) -> Result<HashMap<NaiveDate, Vec<CurrencyAmount>>> {
        let balances = sqlx::query!(
            r#"
            SELECT DATE_TRUNC('month', t.date)::date AS "month!", c.code, c.minor_units, COALESCE(SUM(e.amount), 0) AS "amount!"
            FROM transaction_entry e
                LEFT JOIN transaction t ON t.id = e.transaction_id
                LEFT JOIN account a ON a.id = e.account_id
                LEFT JOIN currency c ON c.code = e.currency
            WHERE t.user_id = $1
                AND (a.name = $2 OR a.name LIKE $2 || ':%')
                AND t.date >= DATE_TRUNC('month', now() - INTERVAL '1 year')
            GROUP BY DATE_TRUNC('month', t.date), c.code
            ORDER BY "month!"
            "#,
            user_id,
            account_name,
        )
        .fetch_all(&*self.0)
        .await?;

        let mut result: HashMap<NaiveDate, Vec<CurrencyAmount>> = HashMap::default();
        for record in balances {
            let currency = Currency::new(record.code, record.minor_units.try_into().unwrap_or(0));
            let amount =
                CurrencyAmount::from_minor(currency, record.amount.try_into().unwrap_or(0));

            result
                .entry(record.month)
                .and_modify(|amounts| amounts.push(amount.clone()))
                .or_insert_with(move || vec![amount]);
        }

        Ok(result)
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

    async fn periodic_cumulative_balance(
        &self,
        user_id: &str,
        account: &str,
        interval: ReportInterval,
    ) -> Result<HashMap<String, InstantBalances>> {
        let interval_value = match interval {
            ReportInterval::Daily => "day",
            ReportInterval::Monthly => "month",
            ReportInterval::Weekly => "week",
        };

        let balances = sqlx::query!(
            r#"
            SELECT "date!", code, minor_units, "amount!"
            FROM (
                SELECT
                    DATE_TRUNC($3, t.date)::date AS "date!",
                    c.code,
                    c.minor_units,
                    COALESCE(SUM(e.amount) OVER (PARTITION BY c.code ORDER BY DATE_TRUNC($3, t.date)), 0) AS "amount!"
                FROM transaction_entry e
                    LEFT JOIN transaction t ON t.id = e.transaction_id
                    LEFT JOIN account a ON a.id = e.account_id
                    LEFT JOIN currency c ON c.code = e.currency
                WHERE t.user_id = $1
                    AND (a.name = $2 OR a.name LIKE $2 || ':%')
                ORDER BY "date!"
            ) AS sums
            WHERE "date!" >= DATE_TRUNC($3, NOW() - INTERVAL '1 year')
            GROUP BY "date!", code, minor_units, "amount!"
            ORDER BY "date!"
            "#,
            user_id,
            account,
            interval_value
        )
        .fetch_all(&*self.0)
        .await?;

        let mut balances_by_code: HashMap<String, InstantBalances> = HashMap::new();
        for record in balances {
            let currency = Currency::new(record.code, record.minor_units.try_into()?);
            let amount: i32 = record.amount.try_into()?;

            balances_by_code
                .entry(currency.code().to_owned())
                .and_modify(|amounts| amounts.push(record.date, amount))
                .or_insert_with(|| {
                    InstantBalances::new_with_balance(currency, record.date, amount)
                });
        }

        Ok(balances_by_code)
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
