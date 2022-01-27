use crate::{
    ledger::{domain, models},
    schema, PostgresConn,
};

use anyhow::Context;
use tracing::{debug, error, info};
use uuid::Uuid;

use super::{Commands, UpdateTransactionError};

pub struct PostgresCommands<'a>(pub &'a PostgresConn);

#[async_trait]
impl<'a> Commands for PostgresCommands<'a> {
    async fn delete_transaction(&self, owner_id: Uuid, transaction_id: Uuid) -> anyhow::Result<()> {
        use diesel::prelude::*;
        use schema::transaction::dsl::*;

        self.0
            .run(move |conn| {
                diesel::delete(transaction.filter(user_id.eq(owner_id).and(id.eq(transaction_id))))
                    .execute(conn)
            })
            .await
            .map(|count| {
                info!(user_id = %owner_id, %transaction_id, rows = count, "Deleted transaction.");
            })
            .map_err(anyhow::Error::from)
    }

    async fn persist_transaction(
        &self,
        transaction: domain::transactions::NewTransaction,
    ) -> anyhow::Result<domain::transactions::Transaction> {
        use diesel::prelude::*;

        let transaction_model: models::NewTransaction = (&transaction).into();

        let saved_transaction = self
            .0
            .run(move |conn| {
                conn.build_transaction()
                    .run::<models::Transaction, diesel::result::Error, _>(|| {
                        let saved_transaction: models::Transaction =
                            diesel::insert_into(schema::transaction::table)
                                .values(transaction_model)
                                .get_result(conn)?;

                        let entry_models = models::NewTransactionEntry::from_domain_entries(
                            saved_transaction.id,
                            transaction.user_id(),
                            transaction.entries(),
                        )
                        .map_err(|error| {
                            error!(?error, "Failed to map transaction entries to model.");

                            diesel::result::Error::RollbackTransaction
                        })?;

                        diesel::insert_into(schema::transaction_entry::table)
                            .values(entry_models)
                            .execute(conn)?;

                        Ok(saved_transaction)
                    })
            })
            .await?;

        info!(id = %saved_transaction.id, "Persisted new transaction.");

        let transaction_id = saved_transaction.id;
        let entries: Vec<models::FullTransactionEntry> = self
            .0
            .run(move |conn| {
                schema::transaction_entry::table
                    .inner_join(schema::account::table)
                    .inner_join(schema::currency::table)
                    .filter(schema::transaction_entry::transaction_id.eq(transaction_id))
                    .load(conn)
            })
            .await?;

        Ok(saved_transaction.try_into_domain(&entries)?)
    }

    async fn update_transaction(
        &self,
        transaction_id: Uuid,
        update: domain::transactions::NewTransaction,
    ) -> Result<domain::transactions::Transaction, UpdateTransactionError> {
        let transaction_changeset = models::NewTransaction::from(&update);
        let transaction_entries = models::NewTransactionEntry::from_domain_entries(
            transaction_id,
            transaction_changeset.user_id,
            update.entries(),
        )
        .context("Failed to convert domain entries to model.")?;

        let (saved_transaction, saved_entries) = self
            .0
            .run::<_, Result<_, UpdateTransactionError>>(move |conn| {
                use diesel::prelude::*;

                let updated_transaction = conn
                    .build_transaction()
                    .run::<_, UpdateTransactionError, _>(|| {
                        let updated_transaction: models::Transaction =
                            diesel::update(schema::transaction::table.filter(
                                schema::transaction::id.eq(transaction_id).and(
                                    schema::transaction::user_id.eq(transaction_changeset.user_id),
                                ),
                            ))
                            .set(transaction_changeset)
                            .get_result(conn)
                            .map_err(|err| {
                                debug!(%transaction_id, "Rolling back transaction update because the transaction does not exist.");

                                err
                            })?;

                        diesel::delete(
                            schema::transaction_entry::table.filter(
                                schema::transaction_entry::transaction_id.eq(transaction_id),
                            ),
                        )
                        .execute(conn)?;

                        diesel::insert_into(schema::transaction_entry::table)
                            .values(&transaction_entries)
                            .execute(conn)?;

                        Ok(updated_transaction)
                    })?;

                let entries = models::TransactionEntry::belonging_to(&updated_transaction)
                    .inner_join(schema::account::table)
                    .inner_join(schema::currency::table)
                    .get_results::<models::FullTransactionEntry>(conn)?;

                Ok((updated_transaction, entries))
            })
            .await?;

        info!(%transaction_id, "Updated transaction.");

        Ok(saved_transaction
            .try_into_domain(&saved_entries)
            .context("Failed to convert transaction model into domain object.")?)
    }
}

impl From<anyhow::Error> for UpdateTransactionError {
    fn from(error: anyhow::Error) -> Self {
        Self::Unknown(error)
    }
}

impl From<diesel::result::Error> for UpdateTransactionError {
    fn from(error: diesel::result::Error) -> Self {
        match error {
            diesel::result::Error::NotFound => Self::TransactionNotFound,
            other => Self::DatabaseError(other.into()),
        }
    }
}
