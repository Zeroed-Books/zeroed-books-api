use std::convert::TryInto;

use crate::{
    ledger::{domain, models},
    schema, PostgresConn,
};
use tracing::{error, info};

use super::Commands;

pub struct PostgresCommands<'a>(pub &'a PostgresConn);

#[async_trait]
impl<'a> Commands for PostgresCommands<'a> {
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

        let domain_entries = entries
            .iter()
            .map(|entry| entry.try_into())
            .collect::<anyhow::Result<Vec<domain::transactions::TransactionEntry>>>()?;

        Ok(domain::transactions::Transaction {
            id: saved_transaction.id,
            user_id: saved_transaction.user_id,
            date: saved_transaction.date,
            payee: saved_transaction.payee,
            notes: if saved_transaction.notes.is_empty() {
                None
            } else {
                Some(saved_transaction.notes)
            },
            entries: domain_entries,
            created_at: saved_transaction.created_at,
            updated_at: saved_transaction.updated_at,
        })
    }
}
