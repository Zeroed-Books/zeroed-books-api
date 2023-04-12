use chrono::NaiveDate;

use super::currency::Currency;

/// A collection of instant balances associated with a specific currency.
pub struct InstantBalances {
    currency: Currency,
    balances: Vec<InstantBalance>,
}

/// An account balance at a certain instant in time.
pub struct InstantBalance {
    instant: NaiveDate,
    amount: i32,
}

impl InstantBalances {
    /// Create a new collection of balances.
    ///
    /// # Arguments
    /// * `currency` - The currency associated with the amounts.
    pub fn new(currency: Currency) -> Self {
        Self {
            currency,
            balances: vec![],
        }
    }

    pub fn new_with_balance(currency: Currency, instant: NaiveDate, amount: i32) -> Self {
        Self {
            currency,
            balances: vec![InstantBalance { instant, amount }],
        }
    }

    /// Add a new balance to the collection.
    ///
    /// # Arguments
    /// * `instant` - The instant that the balance represents.
    /// * `amount` - The balance at the given instant.
    pub fn push(&mut self, instant: NaiveDate, amount: i32) {
        self.balances.push(InstantBalance { instant, amount })
    }

    pub fn currency(&self) -> &Currency {
        &self.currency
    }

    pub fn balances(&self) -> &[InstantBalance] {
        &self.balances
    }
}

impl InstantBalance {
    pub fn instant(&self) -> NaiveDate {
        self.instant
    }

    pub fn amount(&self) -> i32 {
        self.amount
    }
}
