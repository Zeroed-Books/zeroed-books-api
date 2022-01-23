use tracing::{debug, trace};
use uuid::Uuid;

use crate::{
    ledger::{domain, models},
    schema, PostgresConn,
};

use super::Queries;

pub struct PostgresQueries<'a>(pub &'a PostgresConn);

type TransactionWithEntries = (models::Transaction, Vec<models::FullTransactionEntry>);

#[async_trait]
impl<'a> Queries for PostgresQueries<'a> {
    async fn get_transaction(
        &self,
        user_id: Uuid,
        transaction_id: Uuid,
    ) -> anyhow::Result<Option<domain::transactions::Transaction>> {
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

    async fn latest_transactions(
        &self,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<domain::transactions::Transaction>> {
        use diesel::prelude::*;

        let transaction_data: Vec<TransactionWithEntries> = self
            .0
            .run::<_, anyhow::Result<_>>(move |conn| {
                let mut transactions = schema::transaction::table
                    .filter(schema::transaction::user_id.eq(user_id))
                    .order((
                        schema::transaction::date.desc(),
                        schema::transaction::created_at.asc(),
                    ))
                    .limit(50)
                    .load::<models::Transaction>(conn)?;

                let entries = models::TransactionEntry::belonging_to(&transactions)
                    .inner_join(schema::account::table)
                    .inner_join(schema::currency::table)
                    .order(schema::transaction_entry::order)
                    .load::<models::FullTransactionEntry>(conn)?
                    .grouped_by(&transactions);

                Ok(transactions.drain(..).zip(entries).collect::<_>())
            })
            .await?;

        Ok(transaction_data
            .iter()
            .map(|(transaction, entries)| transaction.try_into_domain(entries))
            .collect::<anyhow::Result<Vec<domain::transactions::Transaction>>>()?)
    }
}
