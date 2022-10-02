use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use sqlx::{FromRow, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::{
    database::PostgresConnection,
    ledger::{
        domain::transactions::{Transaction, TransactionCursor},
        models,
    },
};

/// Query parameters for listing transactions.
#[derive(Default)]
pub struct TransactionQuery {
    /// The owner of the transactions to search for.
    pub user_id: Uuid,
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
    pub items: Vec<Transaction>,
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

        let mut transactions_data: Vec<models::Transaction> = query_builder
            .build()
            .fetch_all(&**self)
            .await?
            .iter()
            .map(models::Transaction::from_row)
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
        .fetch_all(&**self)
        .await?;

        let mut entries_by_transaction: HashMap<Uuid, Vec<models::FullTransactionEntry>> =
            HashMap::new();
        for entry in entries {
            entries_by_transaction
                .entry(entry.entry.transaction_id)
                .and_modify(|entries| entries.push(entry.clone()))
                .or_insert_with(|| vec![entry]);
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
            .collect::<anyhow::Result<Vec<_>>>()?;

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
