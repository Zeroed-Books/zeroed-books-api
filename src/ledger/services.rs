use std::{collections::HashMap, convert::TryFrom};

use anyhow::Result;
use chrono::NaiveDate;

use crate::repos::transactions::{DynTransactionRepo, TransactionQuery};

use super::{
    domain::{
        currency::CurrencyAmount,
        reports::InstantBalances,
        transactions::{Transaction, TransactionCursor},
    },
    queries::{DynAccountQueries, ReportInterval},
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
    pub async fn account_periodic_balance(
        &self,
        user_id: &str,
        account: &str,
        balance_type: AccountBalanceType,
        interval: ReportInterval,
    ) -> Result<HashMap<String, InstantBalances>> {
        match balance_type {
            AccountBalanceType::Cummulative => {
                self.account_queries
                    .periodic_cumulative_balance(user_id, account, interval)
                    .await
            }
        }
    }

    pub async fn get_monthly_account_balance(
        &self,
        user_id: &str,
        account_name: &str,
    ) -> Result<HashMap<NaiveDate, Vec<CurrencyAmount>>> {
        self.account_queries
            .get_monthly_balance(user_id, account_name)
            .await
    }

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

#[derive(Debug)]
pub enum AccountBalanceType {
    Cummulative,
}
