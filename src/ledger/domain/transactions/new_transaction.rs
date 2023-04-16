use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
};

use chrono::NaiveDate;
use tracing::{debug, error, trace};
use validator::{Validate, ValidationError, ValidationErrors, ValidationErrorsKind};

use super::{
    new_transaction_data::NewTransactionData,
    new_transaction_entry_data::{NewTransactionEntryAmountData, NewTransactionEntryData},
};

/// A new transaction that has not been persisted yet.
#[derive(Debug, PartialEq)]
pub struct NewTransaction {
    user_id: String,
    date: NaiveDate,
    payee: String,
    notes: Option<String>,
    entries: Vec<NewTransactionEntry>,
}

/// A new transaction entry that has not been persisted yet.
#[derive(Debug, PartialEq)]
pub struct NewTransactionEntry {
    account: String,
    amount: NewTransactionEntryAmount,
}

/// A monetary amount for a new transaction.
#[derive(Debug, PartialEq)]
pub struct NewTransactionEntryAmount {
    currency: String,
    value: i32,
}

/// Attempt to auto-balance the entries of a transaction.
///
/// In the case where there is exactly one entry that does not specify an
/// amount, its amount will be set to the outstanding balance of the other
/// entries.
///
/// In all other cases, no changes are made to the entries.
fn try_balance(entries: &mut [NewTransactionEntryData]) {
    let mut balancing_entry: Option<&mut NewTransactionEntryData> = None;
    let mut currency_sums: HashMap<String, i32> = HashMap::new();

    // Sum all the entries to find any outstanding (non-zero) balances.
    for entry in entries {
        match &entry.amount {
            Some(amount) => {
                let current_value = currency_sums.entry(amount.currency.clone()).or_insert(0);

                *current_value += amount.value;
            }
            None => {
                if balancing_entry.is_some() {
                    // There's already an empty entry that would be used to
                    // auto-balance. We can't have two.
                    debug!(
                        ?entry,
                        ?balancing_entry,
                        "Can't auto-balance entries due to two entries with no amount."
                    );

                    return;
                }

                balancing_entry = Some(entry);
            }
        }
    }

    if let Some(entry) = balancing_entry {
        let unbalanced_sums: Vec<(&String, &i32)> =
            currency_sums.iter().filter(|(_, sum)| **sum != 0).collect();

        // We can only auto-balance if there's exactly one currency with an
        // outstanding balance.
        if unbalanced_sums.len() != 1 {
            debug!(
                ?unbalanced_sums,
                "Cannot balance entries due to multiple unbalanced currencies."
            );

            return;
        }

        let unbalanced_sum = unbalanced_sums[0];
        let balancing_amount = -unbalanced_sum.1;

        entry.amount = Some(NewTransactionEntryAmountData {
            currency: unbalanced_sum.0.clone(),
            value: balancing_amount,
        });
    }
}

impl NewTransaction {
    /// Construct a new transaction from a set of input data.
    ///
    /// The transaction will only be constructed if the input data meets all the
    /// validation rules.
    ///
    /// # Arguments
    /// * `user_id` - The ID of the user who owns the transaction.
    /// * `data` - The input data describing the transaction.
    ///
    /// # Returns
    /// The new transaction if the data is valid, or a set of
    ///  [`ValidationErrors`] otherwise.
    pub fn from_data<S: Into<String>>(
        user_id: S,
        mut data: NewTransactionData,
    ) -> Result<Self, ValidationErrors> {
        try_balance(&mut data.entries);

        match data.validate() {
            Err(validation_error) => {
                debug!(?validation_error, "New transaction failed validation.");

                Err(validation_error)
            }
            Ok(_) => {
                trace!("New transaction passed validation.");

                let entries = data
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(index, data_entry)| {
                        if let Some(ref amount) = data_entry.amount {
                            Ok(NewTransactionEntry {
                                account: data_entry.account.clone(),
                                amount: NewTransactionEntryAmount {
                                    currency: amount.currency.clone(),
                                    value: amount.value,
                                },
                            })
                        } else {
                            // Since we already validated and balanced the
                            // entries, there shouldn't be any entries without
                            // an amount.
                            error!(
                                transaction = ?data,
                                entry = ?data_entry,
                                "Entry amount should not be empty for a validated transaction.",
                            );

                            Err((
                                index,
                                ValidationError {
                                    code: Cow::from("required"),
                                    message: None,
                                    params: HashMap::default(),
                                },
                            ))
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|(index, error)| {
                        let mut entries_list_errors: BTreeMap<usize, Box<ValidationErrors>> =
                            BTreeMap::new();
                        let mut entry_errors = ValidationErrors::new();
                        entry_errors.add("amount", error);

                        entries_list_errors.insert(index, Box::new(entry_errors));

                        let mut errors = ValidationErrors::new();
                        errors
                            .errors_mut()
                            .insert("entries", ValidationErrorsKind::List(entries_list_errors));

                        errors
                    })?;

                Ok(Self {
                    user_id: user_id.into(),
                    date: data.date,
                    payee: data.payee,
                    notes: data.notes,
                    entries,
                })
            }
        }
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    pub fn date(&self) -> NaiveDate {
        self.date
    }

    pub fn payee(&self) -> &str {
        &self.payee
    }

    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    pub fn entries(&self) -> &[NewTransactionEntry] {
        &self.entries
    }
}

impl NewTransactionEntry {
    pub fn account(&self) -> &str {
        &self.account
    }

    pub fn amount(&self) -> &NewTransactionEntryAmount {
        &self.amount
    }
}

impl NewTransactionEntryAmount {
    pub fn currency(&self) -> &str {
        &self.currency
    }

    pub fn value(&self) -> i32 {
        self.value
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn new_auto_balanced_transaction_single_currency() {
        let user_id = "user-id".to_owned();
        let date = NaiveDate::from_ymd_opt(2023, 4, 15).unwrap();
        let payee = "Gas".to_owned();
        let notes = None;

        let data = NewTransactionData {
            date: date,
            payee: payee.clone(),
            notes: notes.clone(),
            entries: vec![
                NewTransactionEntryData {
                    account: "Expenses:Gas".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "USD".to_owned(),
                        value: 2783,
                    }),
                },
                NewTransactionEntryData {
                    account: "Liabilities:Credit".to_owned(),
                    amount: None,
                },
            ],
        };

        let want_transaction = NewTransaction {
            user_id: user_id.clone(),
            date: date,
            payee: payee,
            notes: notes,
            entries: vec![
                NewTransactionEntry {
                    account: "Expenses:Gas".to_owned(),
                    amount: NewTransactionEntryAmount {
                        currency: "USD".to_owned(),
                        value: 2783,
                    },
                },
                NewTransactionEntry {
                    account: "Liabilities:Credit".to_owned(),
                    amount: NewTransactionEntryAmount {
                        currency: "USD".to_owned(),
                        value: -2783,
                    },
                },
            ],
        };

        let got_transaction = NewTransaction::from_data(user_id, data).expect("should be valid");

        assert_eq!(want_transaction, got_transaction);
    }

    #[test]
    fn new_auto_balanced_transaction_multi_currency() {
        let user_id = "user-id".to_owned();
        let date = NaiveDate::from_ymd_opt(2023, 4, 15).unwrap();
        let payee = "Gas".to_owned();
        let notes = None;

        let data = NewTransactionData {
            date: date,
            payee: payee.clone(),
            notes: notes.clone(),
            entries: vec![
                NewTransactionEntryData {
                    account: "Expenses:Gas".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "USD".to_owned(),
                        value: 2783,
                    }),
                },
                NewTransactionEntryData {
                    account: "Liabilities:Credit".to_owned(),
                    amount: None,
                },
                NewTransactionEntryData {
                    account: "Expenses:Food".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "EUR".to_owned(),
                        value: 543,
                    }),
                },
                NewTransactionEntryData {
                    account: "Liabilities:Credit".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "EUR".to_owned(),
                        value: -543,
                    }),
                },
            ],
        };

        let want_transaction = NewTransaction {
            user_id: user_id.clone(),
            date: date,
            payee: payee,
            notes: notes,
            entries: vec![
                NewTransactionEntry {
                    account: "Expenses:Gas".to_owned(),
                    amount: NewTransactionEntryAmount {
                        currency: "USD".to_owned(),
                        value: 2783,
                    },
                },
                NewTransactionEntry {
                    account: "Liabilities:Credit".to_owned(),
                    amount: NewTransactionEntryAmount {
                        currency: "USD".to_owned(),
                        value: -2783,
                    },
                },
                NewTransactionEntry {
                    account: "Expenses:Food".to_owned(),
                    amount: NewTransactionEntryAmount {
                        currency: "EUR".to_owned(),
                        value: 543,
                    },
                },
                NewTransactionEntry {
                    account: "Liabilities:Credit".to_owned(),
                    amount: NewTransactionEntryAmount {
                        currency: "EUR".to_owned(),
                        value: -543,
                    },
                },
            ],
        };

        let got_transaction = NewTransaction::from_data(user_id, data).expect("should be valid");

        assert_eq!(want_transaction, got_transaction);
    }

    #[test]
    fn new_auto_balanced_transaction_single_currency_zero_amount() {
        let user_id = "user-id".to_owned();
        let date = NaiveDate::from_ymd_opt(2023, 4, 15).unwrap();
        let payee = "Gas".to_owned();
        let notes = None;

        let data = NewTransactionData {
            date: date,
            payee: payee.clone(),
            notes: notes.clone(),
            entries: vec![
                NewTransactionEntryData {
                    account: "Expenses:Gas".to_owned(),
                    amount: Some(NewTransactionEntryAmountData {
                        currency: "USD".to_owned(),
                        value: 0,
                    }),
                },
                NewTransactionEntryData {
                    account: "Liabilities:Credit".to_owned(),
                    amount: None,
                },
            ],
        };

        let error = NewTransaction::from_data(user_id, data)
            .expect_err("should error for missing amount and no outstanding balance");
        let errors = error.errors();

        assert_eq!(
            1,
            errors.len(),
            "Expected 1 field error, received {:?}",
            error
        );
        assert!(
            errors.contains_key("entries"),
            "Expected `entries` key in {:?}",
            errors
        );

        let entries_errors = match &errors["entries"] {
            ValidationErrorsKind::List(errors) => errors,
            other => panic!(
                "Received unexpected error type for `entries` key: {:?}",
                other
            ),
        };

        assert!(
            entries_errors.contains_key(&1),
            "Expected an error for the entry at index 1, found {:?}",
            entries_errors
        );

        let entry_errors = entries_errors[&1].field_errors();

        assert!(
            entry_errors.contains_key("amount"),
            "Expected entry to have `amount` error, found: {:?}",
            entry_errors
        );
        assert_eq!(
            "required", entry_errors["amount"][0].code,
            "Expected `required` error, received {:?}",
            entry_errors["amount"][0]
        );
    }
}
