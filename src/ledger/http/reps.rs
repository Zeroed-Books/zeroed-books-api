use serde::{Deserialize, Serialize};

use crate::ledger::domain::{self, currency::CurrencyParseError};

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
