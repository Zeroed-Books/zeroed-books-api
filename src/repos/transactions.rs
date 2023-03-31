use std::sync::Arc;

use async_trait::async_trait;
use sqlx::{FromRow, Postgres, QueryBuilder};

use crate::{
    database::PostgresConnection, ledger::domain::transactions::TransactionCursor, models,
};

/// Query parameters for listing transactions.
#[derive(Default)]
pub struct TransactionQuery {
    /// The owner of the transactions to search for.
    pub user_id: String,
    /// An optional cursor into the transaction list indicating that only
    /// results occurring after the specified position in the list should be
    /// returned.
    pub after: Option<TransactionCursor>,
    /// Only list transactions with at least one entry that references the
    /// specified account.
    pub account: Option<String>,
}

pub struct TransactionCollection {
    pub next: Option<TransactionCursor>,
    pub items: Vec<models::ledger::TransactionWithEntries>,
}

pub type DynTransactionRepo = Arc<dyn TransactionRepo + Send + Sync>;

#[async_trait]
pub trait TransactionRepo {
    /// List the transactions matching the provided query.
    ///
    /// # Arguments
    ///
    /// * `query` - The query parameters used to filter the list.
    ///
    /// # Returns
    ///
    /// An [`anyhow::Result`] containing the transaction collection.
    async fn list_transactions(
        &self,
        query: TransactionQuery,
    ) -> anyhow::Result<TransactionCollection>;
}

const TRANSACTION_PAGE_SIZE: u8 = 50;

#[async_trait]
impl TransactionRepo for PostgresConnection {
    async fn list_transactions(
        &self,
        query: TransactionQuery,
    ) -> anyhow::Result<TransactionCollection> {
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
            // Select one more than the page size so we can determine if there
            // is a next page.
            .push_bind(i16::from(TRANSACTION_PAGE_SIZE) + 1);

        let mut transactions_data: Vec<models::ledger::Transaction> = query_builder
            .build()
            .fetch_all(&**self)
            .await?
            .iter()
            .map(models::ledger::Transaction::from_row)
            .collect::<Result<Vec<_>, sqlx::Error>>()?;

        // To figure out if there is a next page, we query one more element than
        // the maximum page size. If it exists, we remove it from the page, but
        // remember that there are more elements.
        let has_next_page = transactions_data.len() > usize::from(TRANSACTION_PAGE_SIZE);
        if has_next_page {
            transactions_data.pop();
        }

        let transaction_ids = transactions_data.iter().map(|t| t.id).collect::<Vec<_>>();

        let entries = sqlx::query_as!(
            models::ledger::TransactionEntry,
            r#"
            SELECT *
            FROM transaction_entry e
            WHERE e.transaction_id = ANY($1)
            ORDER BY e."order"
            "#,
            &transaction_ids,
        )
        .fetch_all(&**self)
        .await?;

        let account_ids = entries.iter().map(|e| e.account_id).collect::<Vec<_>>();
        let accounts = sqlx::query_as!(
            models::ledger::Account,
            r#"
            SELECT DISTINCT id, user_id, name, created_at
            FROM account a
            WHERE a.id = ANY($1)
            "#,
            &account_ids,
        )
        .fetch_all(&**self)
        .await?;

        let currency_codes = entries
            .iter()
            .map(|e| e.currency.clone())
            .collect::<Vec<_>>();
        let currencies = sqlx::query_as!(
            models::ledger::Currency,
            r#"
            SELECT DISTINCT *
            FROM currency c
            WHERE c.code = ANY($1)
            "#,
            &currency_codes,
        )
        .fetch_all(&**self)
        .await?;

        let cursor = if has_next_page {
            let last_transaction = &transactions_data[transactions_data.len() - 1];

            Some(TransactionCursor {
                after_date: last_transaction.date,
                after_created_at: last_transaction.created_at,
            })
        } else {
            None
        };

        let transactions = models::ledger::TransactionWithEntries::zip_with_entries(
            transactions_data,
            entries,
            currencies,
            accounts,
        )?;

        Ok(TransactionCollection {
            next: cursor,
            items: transactions,
        })
    }
}
