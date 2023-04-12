#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Currency {
    code: String,
    minor_units: u8,
}

#[derive(Debug, Eq, PartialEq)]
pub enum CurrencyParseError {
    /// The provided amount could not be parsed as a number.
    InvalidNumber(String),
    /// The provided amount included more precision than the currency's minor
    /// units allow for. The parameter is the number of
    TooManyDecimals(Currency, usize),
}

impl Currency {
    /// Construct a new currency.
    ///
    /// # Arguments
    /// * `code` - The currency's unique string code.
    /// * `minor_units` - The number of decimal places allowed by the currency.
    ///
    /// # Examples
    ///
    /// ```
    /// # use zeroed_books_api::ledger::domain::currency::Currency;
    /// let _usd = Currency::new("USD".to_owned(), 2);
    /// let _jpy = Currency::new("JPY".to_owned(), 0);
    /// ```
    pub fn new(code: String, minor_units: u8) -> Self {
        Self { code, minor_units }
    }

    /// Parse an amount from a string representation.
    ///
    /// # Arguments
    /// * `raw_amount` - A string containing a numeric amount. This can include
    ///   whitespace and separators.
    ///
    /// # Returns
    ///
    /// The parsed amount as an integer in the currency's minor units. This can
    /// be represented as `amount * 10^n` where `n` is the currency's minor
    /// units.
    ///
    /// The amount is always represented as an integer so that we do not have to
    /// deal with floating point precision errors.
    pub fn parse_amount(&self, raw_amount: &str) -> Result<i32, CurrencyParseError> {
        let decimal = ".";
        let separator = ",";

        let cleaned_amount = raw_amount.replace(separator, "").replace(' ', "");

        let number_to_parse = match cleaned_amount.rsplit_once(decimal) {
            // The number has no decimals, so pad it with the appropriate number
            // of zeroes for the currency.
            None => format!("{}{}", cleaned_amount, "0".repeat(self.minor_units.into())),

            // The number includes a decimal component, so validate that it does
            // not contain too many decimal places.
            Some((whole_part, decimal_part)) => {
                if decimal_part.len() <= Into::<usize>::into(self.minor_units) {
                    format!(
                        "{}{:0<width$}",
                        whole_part,
                        decimal_part,
                        width = self.minor_units.into(),
                    )
                } else {
                    return Err(CurrencyParseError::TooManyDecimals(
                        self.clone(),
                        decimal_part.len(),
                    ));
                }
            }
        };

        number_to_parse
            .parse()
            .map_err(|_| CurrencyParseError::InvalidNumber(raw_amount.to_owned()))
    }

    pub fn format_value(&self, value: i32) -> String {
        // Preserve the sign, but then do string manipulation on the absolute
        // value so we don't have to worry about a negative sign.
        let sign = if value.is_negative() { "-" } else { "" };
        let amount_str = value.abs().to_string();

        // We have to pad the value in order to ensure the string is long enough
        // to insert the decimal point at the appropriate location.
        let padded = format!(
            "{:0>width$}",
            amount_str,
            width = usize::from(self.minor_units) + 1
        );
        let decimal_location = padded.len() - usize::from(self.minor_units);

        let whole_part = &padded[..decimal_location];
        let decimal_part = &padded[decimal_location..];

        format!("{}{}.{}", sign, whole_part, decimal_part)
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn minor_units(&self) -> u8 {
        self.minor_units
    }
}

/// An amount associated with a specific currency.
///
/// The amount is always stored as a whole number, so the value depends on the
/// associated currency's minor units.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrencyAmount {
    currency: Currency,
    value: i32,
}

impl CurrencyAmount {
    pub fn from_minor(currency: Currency, value: i32) -> Self {
        Self { currency, value }
    }

    pub fn from_str(currency: Currency, raw_amount: &str) -> Result<Self, CurrencyParseError> {
        let value = currency.parse_amount(raw_amount)?;

        Ok(Self { currency, value })
    }

    pub fn currency(&self) -> &Currency {
        &self.currency
    }

    pub fn value(&self) -> i32 {
        self.value
    }

    pub fn format_value(&self) -> String {
        self.currency.format_value(self.value)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn test_currency(minor_units: u8) -> Currency {
        match minor_units {
            0 => Currency::new("JPY".to_owned(), 0),
            1 => Currency::new("XX1".to_owned(), 1),
            2 => Currency::new("USD".to_owned(), 2),
            units => Currency::new("XXM".to_owned(), units),
        }
    }

    #[test]
    fn format_value_longer_than_padding() {
        let currency = test_currency(2);
        let amount = CurrencyAmount::from_minor(currency, 12345);
        let want_formatted = "123.45";

        let formatted = amount.format_value();

        assert_eq!(want_formatted, formatted);
    }

    #[test]
    fn format_value_with_only_tens_place() {
        let currency = test_currency(2);
        let amount = CurrencyAmount::from_minor(currency, 70);
        let want_formatted = "0.70";

        let formatted = amount.format_value();

        assert_eq!(want_formatted, formatted);
    }

    #[test]
    fn format_value_with_only_hundreds_place() {
        let currency = test_currency(2);
        let amount = CurrencyAmount::from_minor(currency, 7);
        let want_formatted = "0.07";

        let formatted = amount.format_value();

        assert_eq!(want_formatted, formatted);
    }

    #[test]
    fn format_negative_decimal() {
        let currency = test_currency(2);
        let amount = CurrencyAmount::from_minor(currency, -7);
        let want_formatted = "-0.07";

        let formatted = amount.format_value();

        assert_eq!(want_formatted, formatted);
    }

    #[test]
    fn parse_amount_whole_number_no_minor_units() {
        let currency = test_currency(0);
        let raw_amount = "12";

        let parsed_amount = currency
            .parse_amount(raw_amount)
            .expect("unexpected parsing error");

        assert_eq!(12, parsed_amount);
    }

    #[test]
    fn parse_amount_whole_number_one_minor_unit() {
        let currency = test_currency(1);
        let raw_amount = "12";
        let want_amount = 120;

        let parsed_amount = currency
            .parse_amount(raw_amount)
            .expect("unexpected parsing error");

        assert_eq!(want_amount, parsed_amount);
    }

    #[test]
    fn parse_amount_invalid_number() {
        let currency = test_currency(0);
        let raw_amount = "squirrel";

        let error = currency
            .parse_amount(raw_amount)
            .expect_err("invalid number should return error");

        assert_eq!(
            CurrencyParseError::InvalidNumber(raw_amount.to_owned()),
            error
        );
    }

    #[test]
    fn parse_amount_decimal_one_minor_unit() {
        let currency = test_currency(1);
        let raw_amount = "128.9";
        let want_amount = 1289;

        let parsed_amount = currency
            .parse_amount(raw_amount)
            .expect("failed to parse decimal");

        assert_eq!(want_amount, parsed_amount);
    }

    #[test]
    fn parse_amount_decimal_two_minor_units() {
        let currency = test_currency(2);
        let raw_amount = "128.93";
        let want_amount = 12893;

        let parsed_amount = currency
            .parse_amount(raw_amount)
            .expect("failed to parse decimal");

        assert_eq!(want_amount, parsed_amount);
    }

    #[test]
    fn parse_amount_too_many_decimals_zero_minor_units() {
        let currency = test_currency(0);
        let raw_amount = "1.0";
        let want_error = CurrencyParseError::TooManyDecimals(currency.clone(), 1);

        let error = currency
            .parse_amount(raw_amount)
            .expect_err("invalid number should return error");

        assert_eq!(want_error, error);
    }

    #[test]
    fn parse_amount_too_many_decimals_one_minor_unit() {
        let currency = test_currency(1);
        let raw_amount = "1.00";
        let want_error = CurrencyParseError::TooManyDecimals(currency.clone(), 2);

        let error = currency
            .parse_amount(raw_amount)
            .expect_err("invalid number should return error");

        assert_eq!(want_error, error);
    }

    #[test]
    fn parse_amount_too_many_decimals_five_minor_units() {
        let currency = test_currency(5);
        let raw_amount = "3.141592";
        let want_error = CurrencyParseError::TooManyDecimals(currency.clone(), 6);

        let error = currency
            .parse_amount(raw_amount)
            .expect_err("invalid number should return error");

        assert_eq!(want_error, error);
    }

    #[test]
    fn parse_amount_separator_char() {
        let currency = test_currency(0);
        let raw_amount = "8,675,309";
        let want_amount = 8675309;

        let parsed_amount = currency
            .parse_amount(raw_amount)
            .expect("failed to parse with separators");

        assert_eq!(want_amount, parsed_amount);
    }

    #[test]
    fn parse_amount_separator_whitespace() {
        let currency = test_currency(0);
        let raw_amount = "8 675 309";
        let want_amount = 8675309;

        let parsed_amount = currency
            .parse_amount(raw_amount)
            .expect("failed to parse with separators");

        assert_eq!(want_amount, parsed_amount);
    }

    #[test]
    fn parse_amount_no_whole_digits() {
        let currency = test_currency(1);
        let raw_amount = ".1";
        let want_amount = 1;

        let parsed_amount = currency
            .parse_amount(raw_amount)
            .expect("failed to parse with no whole digits");

        assert_eq!(want_amount, parsed_amount);
    }

    #[test]
    fn parse_amount_zero_as_decimal() {
        let currency = test_currency(3);
        let raw_amount = ".00";
        let want_amount = 0;

        let parsed_amount = currency
            .parse_amount(raw_amount)
            .expect("failed to parse zero with no whole digits");

        assert_eq!(want_amount, parsed_amount);
    }

    #[test]
    fn parse_amount_negative_decimal() {
        let currency = test_currency(3);
        let raw_amount = "-3.142";
        let want_amount = -3142;

        let parsed_amount = currency
            .parse_amount(raw_amount)
            .expect("failed to parse negative decimal");

        assert_eq!(want_amount, parsed_amount);
    }

    #[test]
    fn parse_amount_hundreds_place_decimal() {
        let currency = test_currency(2);
        let raw_amount = ".07";
        let want_amount = 7;

        let parsed_amount = currency
            .parse_amount(raw_amount)
            .expect("failed to parse currency with only hundreds place decimal");

        assert_eq!(want_amount, parsed_amount);
    }
}
