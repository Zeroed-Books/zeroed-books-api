use std::convert::TryFrom;

use anyhow::Result;

use crate::repos::transactions::{DynTransactionRepo, TransactionQuery};

use super::{
    domain::transactions::{Transaction, TransactionCursor},
    queries::DynAccountQueries,
};

#[derive(Clone)]
pub struct LedgerService {
    pub account_queries: DynAccountQueries,
    pub transaction_repo: DynTransactionRepo,
}

pub struct TransactionCollection {
    pub items: Vec<Transaction>,
    pub next: Option<TransactionCursor>,
}

impl LedgerService {
    pub async fn list_active_accounts(&self, user_id: &str) -> Result<Vec<String>> {
        self.account_queries.list_active_accounts(user_id).await
    }

    pub async fn list_transactions(
        &self,
        query: TransactionQuery,
    ) -> Result<TransactionCollection> {
        let mut model_collection = self.transaction_repo.list_transactions(query).await?;

        let transactions: Vec<Transaction> = model_collection
            .items
            .drain(..)
            .map(Transaction::try_from)
            .collect::<Result<_>>()?;

        Ok(TransactionCollection {
            items: transactions,
            next: model_collection.next,
        })
    }
}
