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
