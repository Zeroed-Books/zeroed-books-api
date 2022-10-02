use crate::repos::transactions::{DynTransactionRepo, TransactionCollection, TransactionQuery};

#[derive(Clone)]
pub struct LedgerService {
    transaction_repo: DynTransactionRepo,
}

impl LedgerService {
    pub fn new(transaction_repo: DynTransactionRepo) -> Self {
        Self { transaction_repo }
    }

    pub async fn list_transactions(
        &self,
        query: TransactionQuery,
    ) -> anyhow::Result<TransactionCollection> {
        self.transaction_repo.list_transactions(query).await
    }
}
