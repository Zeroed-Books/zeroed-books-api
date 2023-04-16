use serde::{Deserialize, Serialize};
use validator::Validate;

/// An entry in a new transaction.
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct NewTransactionEntryData {
    /// The account that money is being transferred from/to.
    #[validate(length(min = 1))]
    pub account: String,

    /// The amount of money being transferred. One entry of a transaction may
    /// omit an amount, in which case it will be automatically populated with an
    /// amount that balances the transaction.
    #[validate]
    pub amount: Option<NewTransactionEntryAmountData>,
}

/// An amount of money in a specific currency.
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct NewTransactionEntryAmountData {
    /// The unique currency code.
    #[validate(length(equal = 3))]
    pub currency: String,

    /// The amount as an integer. This is computed by `x * 10^n` where `x` is
    /// the monetary amount, and `n` is the number of significant decimal places
    /// for the currency.
    pub value: i32,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn validate_entry_missing_required_fields() {
        let data = NewTransactionEntryData {
            account: "".to_owned(),
            amount: None,
        };

        let errors = data.validate().expect_err("missing required fields");
        let field_errors = errors.field_errors();

        assert_eq!(1, field_errors.len());
        assert_eq!(1, field_errors["account"].len());
        assert_eq!("length", field_errors["account"][0].code);
    }
}
