use std::collections::{HashMap, HashSet};

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ledger::domain::{
    self, currency::CurrencyParseError, transactions::NewTransactionError,
};

#[derive(Serialize)]
pub struct ResourceCollection<T: Serialize> {
    pub items: Vec<T>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct CurrencyAmount {
    pub currency: String,
    pub value: String,
}

impl From<&domain::currency::CurrencyAmount> for CurrencyAmount {
    fn from(amount: &domain::currency::CurrencyAmount) -> Self {
        Self {
            currency: amount.currency().code().to_owned(),
            value: amount.format_value(),
        }
    }
}

#[derive(Deserialize)]
pub struct NewTransaction {
    pub date: chrono::NaiveDate,
    pub payee: String,
    pub notes: Option<String>,
    pub entries: Vec<NewTransactionEntry>,
}

impl NewTransaction {
    pub fn try_into_domain(
        &self,
        user_id: Uuid,
        currencies: HashMap<String, domain::currency::Currency>,
    ) -> Result<domain::transactions::NewTransaction, TransactionValidationError> {
        let mut parsed_entries = Vec::with_capacity(self.entries.len());
        for new_entry in self.entries.iter() {
            let parsed_amount = match &new_entry.amount {
                None => None,
                Some(amount_rep) => {
                    if let Some(currency) = currencies.get(&amount_rep.currency) {
                        Some(domain::currency::CurrencyAmount::from_str(
                            currency.clone(),
                            &amount_rep.value,
                        )?)
                    } else {
                        return Err(TransactionValidationError {
                            message: Some(format!(
                                "The currency code '{}' is unrecognized.",
                                &amount_rep.currency
                            )),
                        });
                    }
                }
            };

            parsed_entries.push(domain::transactions::NewTransactionEntry {
                account: new_entry.account.clone(),
                amount: parsed_amount,
            });
        }

        Ok(domain::transactions::NewTransaction::new(
            user_id,
            self.date,
            self.payee.clone(),
            self.notes.clone(),
            parsed_entries,
        )?)
    }

    pub fn used_currency_codes(&self) -> HashSet<String> {
        self.entries
            .iter()
            .filter_map(|entry| {
                entry
                    .amount
                    .as_ref()
                    .map(|amount| amount.currency.to_owned())
            })
            .collect()
    }
}

#[derive(Deserialize)]
pub struct NewTransactionEntry {
    pub account: String,
    pub amount: Option<CurrencyAmount>,
}

#[derive(Debug, Serialize)]
pub struct TransactionValidationError {
    pub message: Option<String>,
}

impl From<CurrencyParseError> for TransactionValidationError {
    fn from(error: CurrencyParseError) -> Self {
        match error {
            CurrencyParseError::InvalidNumber(raw_amount) => Self {
                message: Some(format!(
                    "The amount '{}' is not a valid number.",
                    raw_amount
                )),
            },
            CurrencyParseError::TooManyDecimals(currency, decimals) => Self {
                message: Some(format!(
                    "The currency allows {} decimal place(s), but the provided value had {}.",
                    currency.minor_units(),
                    decimals
                )),
            },
        }
    }
}

impl From<NewTransactionError> for TransactionValidationError {
    fn from(error: NewTransactionError) -> Self {
        match error {
            NewTransactionError::Unbalanced(_) => Self {
                message: Some("The entries in the transaction are unbalanced.".to_string()),
            },
        }
    }
}

#[derive(Serialize)]
pub struct Transaction {
    pub id: Uuid,
    pub date: NaiveDate,
    pub payee: String,
    pub notes: String,
    pub entries: Vec<TransactionEntry>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&domain::transactions::Transaction> for Transaction {
    fn from(domain: &domain::transactions::Transaction) -> Self {
        Self {
            id: domain.id,
            date: domain.date,
            payee: domain.payee.to_owned(),
            notes: domain.notes.to_owned(),
            entries: domain.entries.iter().map(|entry| entry.into()).collect(),
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        }
    }
}

#[derive(Serialize)]
pub struct TransactionEntry {
    pub account: String,
    pub amount: CurrencyAmount,
}

impl From<&domain::transactions::TransactionEntry> for TransactionEntry {
    fn from(domain: &domain::transactions::TransactionEntry) -> Self {
        Self {
            account: domain.account().to_string(),
            amount: domain.amount().into(),
        }
    }
}
