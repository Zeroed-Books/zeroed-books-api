use std::{borrow::Cow, collections::HashMap};

use chrono::NaiveDate;
use serde::Deserialize;
use validator::{Validate, ValidationError};

use super::new_transaction_entry_data::NewTransactionEntryData;

/// Data for a new transaction provided by a user.
#[derive(Debug, Deserialize, Validate)]
pub struct NewTransactionData {
    /// The date that the transaction was made.
    pub date: NaiveDate,

    /// The payee that the transaction targets.
    #[validate(length(min = 1))]
    pub payee: String,

    /// Notes providing additional details about the transaction.
    pub notes: Option<String>,

    /// The entries of the transaction detailing the accounts that money was
    /// transferred from/to.
    #[validate]
    #[validate(length(min = 2), custom = "validate_entries_balanced")]
    pub entries: Vec<NewTransactionEntryData>,
}

fn validate_entries_balanced(
    entries: &Vec<NewTransactionEntryData>,
) -> Result<(), ValidationError> {
    // Compute separate sums for each currency so that we can differentiate by
    // currency in the case of an imbalance.
    let mut currency_sums: HashMap<String, i32> = HashMap::new();
    for entry in entries {
        if let Some(ref amount) = entry.amount {
            let current_value = currency_sums.entry(amount.currency.clone()).or_insert(0);

            *current_value += amount.value;
        }
    }

    // Collect any unbalanced currencies with a `currency_` prefix so that
    // clients can present the exact unbalanced amounts.
    let unbalanced_currencies: HashMap<Cow<'_, str>, serde_json::Value> = currency_sums
        .iter()
        .filter(|(_, amount)| **amount != 0)
        .map(|(currency, amount)| {
            (
                Cow::from(format!("currency_{}", currency)),
                (*amount).into(),
            )
        })
        .collect();

    if unbalanced_currencies.is_empty() {
        Ok(())
    } else {
        Err(ValidationError {
            code: "unbalanced".into(),
            message: None,
            params: unbalanced_currencies,
        })
    }
}

#[cfg(test)]
mod test {
    use validator::ValidationErrorsKind;

    use crate::ledger::domain::transactions::new_transaction_entry_data::NewTransactionEntryAmountData;

    use super::*;

    #[test]
    fn transaction_validate_empty_required_fields() {
        let data = NewTransactionData {
            date: NaiveDate::from_ymd_opt(2023, 4, 15).unwrap(),
            payee: "".to_owned(),
            notes: None,
            entries: vec![],
        };

        let errors = data.validate().expect_err("should error for empty fields");
        let field_errors = errors.field_errors();

        assert_eq!(2, field_errors.len());

        assert_eq!(1, field_errors["payee"].len());
        assert_eq!("length", field_errors["payee"][0].code);

        assert_eq!(1, field_errors["entries"].len());
        assert_eq!("length", field_errors["entries"][0].code);
    }

    #[test]
    fn transaction_validate_valid_data() {
        let data = NewTransactionData {
            date: NaiveDate::from_ymd_opt(2023, 4, 15).unwrap(),
            payee: "Groceries".to_owned(),
            notes: None,
            entries: vec![
                NewTransactionEntryData {
                    account: "Expenses:Food".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "USD".to_owned(),
                        value: 100,
                    }),
                },
                NewTransactionEntryData {
                    account: "Assets:Checking".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "USD".to_owned(),
                        value: -100,
                    }),
                },
            ],
        };

        data.validate().expect("should be valid data");
    }

    #[test]
    fn transaction_validate_unbalanced() {
        let data = NewTransactionData {
            date: NaiveDate::from_ymd_opt(2023, 4, 15).unwrap(),
            payee: "Unbalanced".to_owned(),
            notes: None,
            entries: vec![
                NewTransactionEntryData {
                    account: "Expenses:Food".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "USD".to_owned(),
                        value: 100,
                    }),
                },
                NewTransactionEntryData {
                    account: "Assets:Checking".to_owned(),
                    // Unbalanced because both entries are positive. Leaves
                    // $2.00 unaccounted for.
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "USD".to_owned(),
                        value: 100,
                    }),
                },
            ],
        };

        let errors = data
            .validate()
            .expect_err("should error for unbalanced entries");
        let field_errors = errors.field_errors();

        assert_eq!(1, field_errors.len());

        assert_eq!(1, field_errors["entries"].len());
        assert_eq!("unbalanced", field_errors["entries"][0].code);
        assert_eq!(200, field_errors["entries"][0].params["currency_USD"]);
    }

    #[test]
    fn transaction_validate_balanced_multiple_currencies() {
        let data = NewTransactionData {
            date: NaiveDate::from_ymd_opt(2023, 4, 15).unwrap(),
            payee: "Groceries".to_owned(),
            notes: None,
            entries: vec![
                NewTransactionEntryData {
                    account: "Expenses:Food".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "USD".to_owned(),
                        value: 100,
                    }),
                },
                NewTransactionEntryData {
                    account: "Assets:Checking".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "USD".to_owned(),
                        value: -100,
                    }),
                },
                NewTransactionEntryData {
                    account: "Expenses:Food".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "EUR".to_owned(),
                        value: 200,
                    }),
                },
                NewTransactionEntryData {
                    account: "Assets:Checking".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "EUR".to_owned(),
                        value: -200,
                    }),
                },
            ],
        };

        data.validate().expect("should be valid data");
    }

    #[test]
    fn transaction_validate_unbalanced_multiple_currencies() {
        let data = NewTransactionData {
            date: NaiveDate::from_ymd_opt(2023, 4, 15).unwrap(),
            payee: "Unbalanced".to_owned(),
            notes: None,
            entries: vec![
                NewTransactionEntryData {
                    account: "Expenses:Food".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "USD".to_owned(),
                        value: 100,
                    }),
                },
                NewTransactionEntryData {
                    account: "Assets:Checking".to_owned(),
                    // Unbalanced because both entries are positive. Leaves
                    // $2.00 unaccounted for.
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "USD".to_owned(),
                        value: 100,
                    }),
                },
                NewTransactionEntryData {
                    account: "Expenses:Food".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "EUR".to_owned(),
                        value: 200,
                    }),
                },
                NewTransactionEntryData {
                    account: "Assets:Checking".to_owned(),
                    // Unbalanced because both entries are positive. Leaves
                    // €4.00 unaccounted for.
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "EUR".to_owned(),
                        value: 200,
                    }),
                },
            ],
        };

        let errors = data
            .validate()
            .expect_err("should error for unbalanced entries");
        let field_errors = errors.field_errors();

        assert_eq!(1, field_errors.len());

        assert_eq!(1, field_errors["entries"].len());
        assert_eq!("unbalanced", field_errors["entries"][0].code);
        assert_eq!(400, field_errors["entries"][0].params["currency_EUR"]);
        assert_eq!(200, field_errors["entries"][0].params["currency_USD"]);
    }

    #[test]
    fn transaction_validate_missing_account() {
        let data = NewTransactionData {
            date: NaiveDate::from_ymd_opt(2023, 4, 15).unwrap(),
            payee: "Groceries".to_owned(),
            notes: None,
            entries: vec![
                NewTransactionEntryData {
                    account: "Expenses:Food".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "USD".to_owned(),
                        value: 100,
                    }),
                },
                NewTransactionEntryData {
                    account: "".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "USD".to_owned(),
                        value: -100,
                    }),
                },
            ],
        };

        let error = data.validate().expect_err("missing account");
        let errors = error.errors();

        assert_eq!(1, errors.len(), "Expected 1 error, found {:?}", errors);
        assert!(
            errors.contains_key("entries"),
            "Expected 'entries' key in {:?}",
            errors
        );

        let entries_errors = match errors.get("entries") {
            Some(ValidationErrorsKind::List(errors)) => errors,
            other => panic!("Expected to receive list of errors, got {:?}", other),
        };

        assert!(
            entries_errors.contains_key(&1),
            "Expected error for entry with index 1, got {:?}",
            entries_errors
        );
        assert_eq!(1, entries_errors[&1].field_errors().len());
        assert_eq!(1, entries_errors[&1].field_errors()["account"].len());
        assert_eq!(
            "length",
            entries_errors[&1].field_errors()["account"][0].code
        );
    }
}
