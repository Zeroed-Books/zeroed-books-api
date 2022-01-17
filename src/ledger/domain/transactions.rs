use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

use super::currency::{Currency, CurrencyAmount};

/// A new transaction entered by a user. This may only be constructed by calling
/// [`Self::new()`] which prevents construction of unbalanced transactions.
#[derive(Clone, Debug, PartialEq)]
pub struct NewTransaction {
    user_id: Uuid,
    date: NaiveDate,
    payee: String,
    notes: Option<String>,
    entries: Vec<TransactionEntry>,
}

#[derive(Debug, PartialEq)]
pub enum NewTransactionError {
    /// The entries in the transaction are not balanced, ie they do not sum to
    /// zero. The value is a mapping of currencies to balances.
    Unbalanced(HashMap<Currency, i32>),
}

impl NewTransaction {

    /// Construct a new transaction. If the transaction is not balanced, and it
    /// cannot be automatically balanced, an error result is returned.
    ///
    /// # Arguments
    /// * `user_id` - The ID of the user who is creating the transaction.
    /// * `date` - The date the transaction occurred.
    /// * `payee` - Who was paid for the transaction.
    /// * `notes` - Any additional notes to store with the transaction.
    /// * `entries` - The entries associated with the transaction. If there is
    ///   exactly one entry missing an amount, it will be automatically balanced
    ///   for the user.
    pub fn new(
        user_id: Uuid,
        date: NaiveDate,
        payee: String,
        notes: Option<String>,
        mut entries: Vec<NewTransactionEntry>,
    ) -> Result<Self, NewTransactionError> {
        let mut balancing_entry: Option<&mut NewTransactionEntry> = None;
        let mut cannot_be_balanced = false;
        let mut sums = HashMap::new();

        for new_entry in entries.iter_mut() {
            match &new_entry.amount {
                Some(amount) => {
                    let previous_sum = sums.entry(amount.currency().clone()).or_insert(0);

                    *previous_sum += amount.value();
                }
                None => {
                    if balancing_entry.is_none() {
                        balancing_entry = Some(new_entry);
                    } else {
                        cannot_be_balanced = true;
                    }
                }
            }
        }

        if cannot_be_balanced {
            return Err(NewTransactionError::Unbalanced(sums));
        }

        let unbalanced_currencies: Vec<(&Currency, &i32)> =
            sums.iter().filter(|(_, amount)| **amount != 0).collect();

        match (unbalanced_currencies.len(), balancing_entry) {
            (0, _) => (),
            (1, Some(mut entry)) => {
                let currency = unbalanced_currencies[0].0;
                let amount = -unbalanced_currencies[0].1;

                entry.amount = Some(CurrencyAmount::from_minor(currency.clone(), amount));
            }
            _ => {
                return Err(NewTransactionError::Unbalanced(sums));
            }
        };

        let validated_entries = entries
            .iter()
            .map(|new_entry| {
                TransactionEntry::new(
                    new_entry.account.to_string(),
                    // We can unwrap because we validated nothing was missing
                    // already.
                    new_entry.amount.clone().unwrap(),
                )
            })
            .collect();

        Ok(Self {
            user_id,
            date,
            payee,
            notes,
            entries: validated_entries,
        })
    }

    pub fn user_id(&self) -> Uuid {
        self.user_id
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

    pub fn entries(&self) -> &Vec<TransactionEntry> {
        &self.entries
    }
}

pub struct NewTransactionEntry {
    pub account: String,
    pub amount: Option<CurrencyAmount>,
}

pub struct Transaction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub date: NaiveDate,
    pub payee: String,
    pub notes: Option<String>,
    pub entries: Vec<TransactionEntry>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransactionEntry {
    account: String,
    amount: CurrencyAmount,
}

impl TransactionEntry {
    pub fn new(account: String, amount: CurrencyAmount) -> Self {
        Self { account, amount }
    }

    pub fn account(&self) -> &str {
        &self.account
    }

    pub fn amount(&self) -> &CurrencyAmount {
        &self.amount
    }
}

#[cfg(test)]
mod test {

    use super::*;

    fn eur() -> Currency {
        Currency::new("EUR".to_owned(), 2)
    }

    fn usd() -> Currency {
        Currency::new("USD".to_owned(), 2)
    }

    #[test]
    fn new_empty_transaction() {
        let _transaction = NewTransaction::new(
            Uuid::new_v4(),
            NaiveDate::from_ymd(2022, 1, 16),
            "Groceries".to_string(),
            None,
            vec![],
        );
    }

    #[test]
    fn new_balanced_transaction_single_currency() {
        let user_id = Uuid::new_v4();
        let date = NaiveDate::from_ymd(2022, 1, 16);
        let payee = "Gas".to_string();
        let notes = None;

        let e1 = NewTransactionEntry {
            account: "Expenses:Gas".to_string(),
            amount: Some(CurrencyAmount::from_minor(usd(), 2783)),
        };
        let e2 = NewTransactionEntry {
            account: "Liabilities:Credit".to_string(),
            amount: Some(CurrencyAmount::from_minor(usd(), -2783)),
        };

        let want_entries = vec![
            TransactionEntry::new(e1.account.clone(), CurrencyAmount::from_minor(usd(), 2783)),
            TransactionEntry::new(e2.account.clone(), CurrencyAmount::from_minor(usd(), -2783)),
        ];

        let entries = vec![e1, e2];
        let transaction = NewTransaction::new(user_id, date, payee, notes, entries)
            .expect("transaction was malformed");

        assert_eq!(&want_entries, transaction.entries());
    }

    #[test]
    fn new_auto_balanced_transaction_single_currency() {
        let user_id = Uuid::new_v4();
        let date = NaiveDate::from_ymd(2022, 1, 16);
        let payee = "Gas".to_string();
        let notes = None;

        let e1 = NewTransactionEntry {
            account: "Expenses:Gas".to_string(),
            amount: Some(CurrencyAmount::from_minor(usd(), 2783)),
        };
        let e2 = NewTransactionEntry {
            account: "Liabilities:Credit".to_string(),
            amount: None,
        };

        let want_entries = vec![
            TransactionEntry::new(e1.account.clone(), CurrencyAmount::from_minor(usd(), 2783)),
            TransactionEntry::new(e2.account.clone(), CurrencyAmount::from_minor(usd(), -2783)),
        ];

        let entries = vec![e1, e2];
        let transaction = NewTransaction::new(user_id, date, payee, notes, entries)
            .expect("transaction was malformed");

        assert_eq!(&want_entries, transaction.entries());
    }

    #[test]
    fn new_unbalanced_transaction_single_currency() {
        let user_id = Uuid::new_v4();
        let date = NaiveDate::from_ymd(2022, 1, 16);
        let payee = "Gas".to_string();
        let notes = None;

        let e1 = NewTransactionEntry {
            account: "Expenses:Gas".to_string(),
            amount: Some(CurrencyAmount::from_minor(usd(), 1)),
        };
        let e2 = NewTransactionEntry {
            account: "Liabilities:Credit".to_string(),
            amount: Some(CurrencyAmount::from_minor(usd(), 1)),
        };

        let entries = vec![e1, e2];

        let error = NewTransaction::new(user_id, date, payee, notes, entries)
            .expect_err("unbalanced transaction should error");

        let expected_sums = HashMap::from([(usd(), 2)]);

        assert_eq!(NewTransactionError::Unbalanced(expected_sums), error);
    }

    #[test]
    fn new_balanced_transaction_multi_currency() {
        let user_id = Uuid::new_v4();
        let date = NaiveDate::from_ymd(2022, 1, 16);
        let payee = "Exxon".to_string();
        let notes = None;

        let e1 = NewTransactionEntry {
            account: "Expenses:Gas".to_string(),
            amount: Some(CurrencyAmount::from_minor(usd(), 2783)),
        };
        let e2 = NewTransactionEntry {
            account: "Liabilities:Credit".to_string(),
            amount: Some(CurrencyAmount::from_minor(usd(), -2783)),
        };
        let e3 = NewTransactionEntry {
            account: "Expenses:Groceries".to_string(),
            amount: Some(CurrencyAmount::from_minor(eur(), 583)),
        };
        let e4 = NewTransactionEntry {
            account: "Liabilities:Credit".to_string(),
            amount: Some(CurrencyAmount::from_minor(eur(), -583)),
        };

        let want_entries = vec![
            TransactionEntry::new(e1.account.clone(), CurrencyAmount::from_minor(usd(), 2783)),
            TransactionEntry::new(e2.account.clone(), CurrencyAmount::from_minor(usd(), -2783)),
            TransactionEntry::new(e3.account.clone(), CurrencyAmount::from_minor(eur(), 583)),
            TransactionEntry::new(e4.account.clone(), CurrencyAmount::from_minor(eur(), -583)),
        ];

        let entries = vec![e1, e2, e3, e4];
        let transaction = NewTransaction::new(user_id, date, payee, notes, entries)
            .expect("transaction was malformed");

        assert_eq!(&want_entries, transaction.entries());
    }

    #[test]
    fn new_autobalanced_transaction_multi_currency() {
        let user_id = Uuid::new_v4();
        let date = NaiveDate::from_ymd(2022, 1, 16);
        let payee = "Exxon".to_string();
        let notes = None;

        let e1 = NewTransactionEntry {
            account: "Expenses:Gas".to_string(),
            amount: Some(CurrencyAmount::from_minor(usd(), 2783)),
        };
        let e2 = NewTransactionEntry {
            account: "Liabilities:Credit".to_string(),
            amount: Some(CurrencyAmount::from_minor(usd(), -2783)),
        };
        let e3 = NewTransactionEntry {
            account: "Expenses:Groceries".to_string(),
            amount: None,
        };
        let e4 = NewTransactionEntry {
            account: "Liabilities:Credit".to_string(),
            amount: Some(CurrencyAmount::from_minor(eur(), -583)),
        };

        let want_entries = vec![
            TransactionEntry::new(e1.account.clone(), CurrencyAmount::from_minor(usd(), 2783)),
            TransactionEntry::new(e2.account.clone(), CurrencyAmount::from_minor(usd(), -2783)),
            TransactionEntry::new(e3.account.clone(), CurrencyAmount::from_minor(eur(), 583)),
            TransactionEntry::new(e4.account.clone(), CurrencyAmount::from_minor(eur(), -583)),
        ];

        let entries = vec![e1, e2, e3, e4];
        let transaction = NewTransaction::new(user_id, date, payee, notes, entries)
            .expect("transaction was malformed");

        assert_eq!(&want_entries, transaction.entries());
    }

    #[test]
    fn new_unbalanced_transaction_multi_currency() {
        let user_id = Uuid::new_v4();
        let date = NaiveDate::from_ymd(2022, 1, 16);
        let payee = "Exxon".to_string();
        let notes = None;

        let e1 = NewTransactionEntry {
            account: "Expenses:Gas".to_string(),
            amount: Some(CurrencyAmount::from_minor(usd(), 2783)),
        };
        let e2 = NewTransactionEntry {
            account: "Liabilities:Credit".to_string(),
            amount: Some(CurrencyAmount::from_minor(usd(), 0)),
        };
        let e3 = NewTransactionEntry {
            account: "Expenses:Groceries".to_string(),
            amount: Some(CurrencyAmount::from_minor(eur(), 583)),
        };
        let e4 = NewTransactionEntry {
            account: "Liabilities:Credit".to_string(),
            amount: Some(CurrencyAmount::from_minor(eur(), 0)),
        };

        let entries = vec![e1, e2, e3, e4];
        let error = NewTransaction::new(user_id, date, payee, notes, entries)
            .expect_err("unbalanced transaction should error");

        let want_sums = HashMap::from([(eur(), 583), (usd(), 2783)]);

        assert_eq!(NewTransactionError::Unbalanced(want_sums), error);
    }
}
