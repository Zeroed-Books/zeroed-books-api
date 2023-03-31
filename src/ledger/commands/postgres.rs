use crate::ledger::{
    domain,
    models::{self},
};

use anyhow::Context;
use async_trait::async_trait;
use sqlx::{PgPool, Postgres, QueryBuilder};
use tracing::{debug, info};
use uuid::Uuid;

use super::{TransactionCommands, UpdateTransactionError};

pub struct PostgresCommands<'a>(pub &'a PgPool);

#[async_trait]
impl<'a> TransactionCommands for PostgresCommands<'a> {
    async fn delete_transaction(&self, owner_id: &str, transaction_id: Uuid) -> anyhow::Result<()> {
        Ok(sqlx::query!(
            r#"
            DELETE FROM "transaction"
            WHERE user_id = $1 AND id = $2
            "#,
            owner_id,
            transaction_id,
        )
        .execute(self.0)
        .await
        .map(|result| {
            info!(user_id = %owner_id, %transaction_id, rows = result.rows_affected(), "Deleted transaction.");
        })?)
    }

    async fn persist_transaction(
        &self,
        transaction: domain::transactions::NewTransaction,
    ) -> anyhow::Result<domain::transactions::Transaction> {
        let transaction_model: models::NewTransaction = (&transaction).into();

        let mut tx = self.0.begin().await?;

        let persisted_transaction = sqlx::query_as!(
            models::Transaction,
            r#"
            INSERT INTO transaction (user_id, "date", payee, notes)
            VALUES ($1, $2, $3, $4)
            RETURNING id, user_id, date, payee, notes, created_at, updated_at
            "#,
            transaction_model.user_id,
            transaction_model.date,
            transaction_model.payee,
            transaction_model.notes,
        )
        .fetch_one(&mut tx)
        .await?;

        let entry_models = models::NewTransactionEntry::from_domain_entries(
            persisted_transaction.id,
            transaction.user_id().to_owned(),
            transaction.entries(),
        )
        .context("Failed to map transaction entries to model.")?;

        let mut entry_query_builder: QueryBuilder<'_, Postgres> = QueryBuilder::new(
            r#"INSERT INTO transaction_entry (transaction_id, "order", account_id, currency, amount)"#,
        );

        entry_query_builder.push_values(entry_models, |mut b, entry| {
            b.push_bind(entry.transaction_id)
                .push_bind(entry.order)
                .push("get_or_create_account(")
                .push_bind_unseparated(&transaction_model.user_id)
                .push_bind(entry.account.name)
                .push_unseparated(")")
                .push_bind(entry.currency)
                .push_bind(entry.amount);
        });

        entry_query_builder.build().execute(&mut tx).await?;
        tx.commit().await?;

        info!(id = %persisted_transaction.id, "Persisted new transaction.");

        let entries = sqlx::query_as::<_, models::FullTransactionEntry>(
            r#"
            SELECT e.*, a.*, c.*
            FROM transaction_entry e
            LEFT JOIN account a ON e.account_id = a.id
            LEFT JOIN currency c ON e.currency = c.code
            WHERE e.transaction_id = $1
            "#,
        )
        .bind(persisted_transaction.id)
        .fetch_all(self.0)
        .await?;

        Ok(persisted_transaction.try_into_domain(&entries)?)
    }

    async fn update_transaction(
        &self,
        transaction_id: Uuid,
        update: domain::transactions::NewTransaction,
    ) -> Result<domain::transactions::Transaction, UpdateTransactionError> {
        let transaction_changeset = models::NewTransaction::from(&update);
        let transaction_entries = models::NewTransactionEntry::from_domain_entries(
            transaction_id,
            transaction_changeset.user_id.clone(),
            update.entries(),
        )
        .context("Failed to convert domain entries to model.")?;

        let mut tx = self.0.begin().await?;

        let updated_transaction = sqlx::query_as!(
            models::Transaction,
            r#"
            UPDATE transaction
            SET
                date = $3,
                payee = $4,
                notes = $5
            WHERE id = $1 AND user_id = $2
            RETURNING id, user_id, date, payee, notes, created_at, updated_at
            "#,
            transaction_id,
            &transaction_changeset.user_id,
            transaction_changeset.date,
            transaction_changeset.payee,
            transaction_changeset.notes
        )
        .fetch_one(&mut tx)
        .await?;

        let old_entry_delete = sqlx::query!(
            r#"
            DELETE FROM transaction_entry
            WHERE transaction_id = $1
            "#,
            transaction_id
        )
        .execute(&mut tx)
        .await?;
        debug!(%transaction_id, rows = old_entry_delete.rows_affected(), "Cleared out old transaction entries.");

        let mut entry_query_builder: QueryBuilder<'_, Postgres> = QueryBuilder::new(
            r#"INSERT INTO transaction_entry (transaction_id, "order", account_id, currency, amount)"#,
        );

        entry_query_builder.push_values(transaction_entries, |mut b, entry| {
            b.push_bind(entry.transaction_id)
                .push_bind(entry.order)
                .push("get_or_create_account(")
                .push_bind_unseparated(&transaction_changeset.user_id)
                .push_bind(entry.account.name)
                .push(")")
                .push_bind(entry.currency)
                .push_bind(entry.amount);
        });

        entry_query_builder.build().execute(&mut tx).await?;
        tx.commit().await?;

        let updated_entries = sqlx::query_as::<_, models::FullTransactionEntry>(
            r#"
            SELECT e.*, a.*, c.*
            FROM transaction_entry e
            LEFT JOIN account a ON e.account_id = a.id
            LEFT JOIN currency c ON e.currency = c.code
            WHERE e.transaction_id = $1
            "#,
        )
        .bind(transaction_id)
        .fetch_all(self.0)
        .await?;

        info!(%transaction_id, "Updated transaction.");

        Ok(updated_transaction
            .try_into_domain(&updated_entries)
            .context("Failed to convert transaction model into domain object.")?)
    }
}

impl From<anyhow::Error> for UpdateTransactionError {
    fn from(error: anyhow::Error) -> Self {
        Self::Unknown(error)
    }
}

impl From<sqlx::Error> for UpdateTransactionError {
    fn from(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::RowNotFound => Self::TransactionNotFound,
            other => Self::DatabaseError(other.into()),
        }
    }
}
