use std::collections::{HashMap, HashSet};

use anyhow::Result;
use base64::{engine::general_purpose, Engine};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize, Serializer};

use uuid::Uuid;

use crate::ledger::domain::{
    self, currency::CurrencyParseError, transactions::NewTransactionError,
};

#[derive(Serialize)]
pub struct ResourceCollection<T: Serialize, C: Serialize> {
    pub next: Option<C>,
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
        user_id: String,
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

#[derive(Deserialize, Serialize)]
pub struct TransactionCursor {
    pub after_date: NaiveDate,
    pub after_created_at: DateTime<Utc>,
}

impl From<&TransactionCursor> for domain::transactions::TransactionCursor {
    fn from(cursor: &TransactionCursor) -> Self {
        Self {
            after_date: cursor.after_date,
            after_created_at: cursor.after_created_at,
        }
    }
}

impl From<domain::transactions::TransactionCursor> for TransactionCursor {
    fn from(cursor: domain::transactions::TransactionCursor) -> Self {
        Self {
            after_date: cursor.after_date,
            after_created_at: cursor.after_created_at,
        }
    }
}

pub struct EncodedTransactionCursor(pub TransactionCursor);

impl From<domain::transactions::TransactionCursor> for EncodedTransactionCursor {
    fn from(cursor: domain::transactions::TransactionCursor) -> Self {
        Self(cursor.into())
    }
}

impl Serialize for EncodedTransactionCursor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let encoded = format!(
            "{}/{}",
            self.0.after_date.format("%Y-%m-%d"),
            self.0.after_created_at.to_rfc3339()
        );

        serializer.collect_str(&general_purpose::URL_SAFE.encode(encoded))
    }
}

impl<'de> Deserialize<'de> for EncodedTransactionCursor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Vis;
        impl serde::de::Visitor<'_> for Vis {
            type Value = EncodedTransactionCursor;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a base64 encoded transaction cursor")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                let formatted = general_purpose::URL_SAFE
                    .decode(v)
                    .map(String::from_utf8)
                    .map_err(serde::de::Error::custom)?
                    .map_err(serde::de::Error::custom)?;

                match formatted.split_once('/') {
                    Some((str_date, str_created_at)) => {
                        let date = NaiveDate::parse_from_str(str_date, "%Y-%m-%d")
                            .map_err(serde::de::Error::custom)?;
                        let created_at = str_created_at
                            .parse::<DateTime<Utc>>()
                            .map_err(serde::de::Error::custom)?;

                        Ok(EncodedTransactionCursor(TransactionCursor {
                            after_date: date,
                            after_created_at: created_at,
                        }))
                    }
                    None => Err(serde::de::Error::custom("improperly encoded cursor")),
                }
            }
        }

        deserializer.deserialize_str(Vis)
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

#[derive(Debug, Deserialize, Serialize)]
pub struct PeriodicAccountBalances(HashMap<String, CurrencyInstantBalances>);

#[derive(Debug, Deserialize, Serialize)]
pub struct CurrencyInstantBalances {
    pub currency: Currency,
    pub balances: Vec<InstantBalance>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InstantBalance {
    instant: NaiveDate,
    balance: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Currency {
    pub code: String,
    pub minor_units: u8,
}

impl From<HashMap<String, domain::reports::InstantBalances>> for PeriodicAccountBalances {
    fn from(value: HashMap<String, domain::reports::InstantBalances>) -> Self {
        let mut result: HashMap<String, CurrencyInstantBalances> = HashMap::new();
        for (currency_code, balances) in value {
            result.insert(currency_code, balances.into());
        }

        Self(result)
    }
}

impl From<domain::reports::InstantBalances> for CurrencyInstantBalances {
    fn from(value: domain::reports::InstantBalances) -> Self {
        let currency = value.currency();
        let balances = value
            .balances()
            .iter()
            .map(|balance| InstantBalance {
                instant: balance.instant(),
                balance: currency.format_value(balance.amount()),
            })
            .collect();

        Self {
            currency: currency.into(),
            balances,
        }
    }
}

impl From<&domain::currency::Currency> for Currency {
    fn from(value: &domain::currency::Currency) -> Self {
        Self {
            code: value.code().to_owned(),
            minor_units: value.minor_units(),
        }
    }
}
