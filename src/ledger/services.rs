use std::convert::TryFrom;

use crate::repos::transactions::{DynTransactionRepo, TransactionQuery};

use super::domain::transactions::{Transaction, TransactionCursor};

#[derive(Clone)]
pub struct LedgerService {
    transaction_repo: DynTransactionRepo,
}

pub struct TransactionCollection {
    pub items: Vec<Transaction>,
    pub next: Option<TransactionCursor>,
}

impl LedgerService {
    pub fn new(transaction_repo: DynTransactionRepo) -> Self {
        Self { transaction_repo }
    }

    pub async fn list_transactions(
        &self,
        query: TransactionQuery,
    ) -> anyhow::Result<TransactionCollection> {
        let mut model_collection = self.transaction_repo.list_transactions(query).await?;

        let transactions: Vec<Transaction> = model_collection
            .items
            .drain(..)
            .map(Transaction::try_from)
            .collect::<anyhow::Result<_>>()?;

        Ok(TransactionCollection {
            items: transactions,
            next: model_collection.next,
        })
    }
}
