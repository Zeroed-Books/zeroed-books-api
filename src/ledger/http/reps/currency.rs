use serde::{Deserialize, Serialize};

use crate::ledger::domain;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Currency {
    pub code: String,
    pub minor_units: u8,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct CurrencyAmount {
    pub currency: Currency,
    pub value: i32,
}

impl From<&domain::currency::Currency> for Currency {
    fn from(value: &domain::currency::Currency) -> Self {
        Self {
            code: value.code().to_owned(),
            minor_units: value.minor_units(),
        }
    }
}

impl From<&domain::currency::CurrencyAmount> for CurrencyAmount {
    fn from(amount: &domain::currency::CurrencyAmount) -> Self {
        Self {
            currency: amount.currency().into(),
            value: amount.value(),
        }
    }
}
