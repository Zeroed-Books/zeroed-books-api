use serde::{Deserialize, Serialize};

use crate::ledger::domain::{self};

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
