pub mod clients;

#[derive(Debug, PartialEq)]
pub struct Email {
    provided_address: String,
    normalized_address: String,
}

impl Email {
    /// Parse an email address from a string. The only requirement is that the
    /// email contains an "@" symbol.
    ///
    /// # Arguments
    ///
    /// * `raw_email` - The email address to parse.
    ///
    /// # Return Value
    ///
    /// Parsing returns a result with parsed email representation if the address
    /// was valid or an empty error if it was not. An error value implies the
    /// address is missing an "@" symbol.
    pub fn parse(raw_email: &str) -> Result<Email, ()> {
        // Email addresses may have multiple "@" symbols, and the last one
        // delimits the local part from the domain.
        let parts = raw_email.rsplit_once('@');

        if let Some((local_part, domain)) = parts {
            return Ok(Email {
                provided_address: raw_email.to_owned(),
                // The only part of an email address that is case insensitive is
                // the domain.
                normalized_address: format!("{}@{}", local_part, domain.to_lowercase()),
            });
        }

        Err(())
    }

    pub fn provided_address(&self) -> &str {
        &self.provided_address
    }

    pub fn normalized_address(&self) -> &str {
        &self.normalized_address
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_missing_at_symbol() {
        let parsed = Email::parse("missing-an-at-symbol");

        assert!(parsed.is_err());
    }

    #[test]
    fn parse_valid_missing_domain() {
        let parsed = Email::parse("someone@").expect("Parse failed");

        assert_eq!("someone@", parsed.provided_address());
        assert_eq!("someone@", parsed.normalized_address());
    }

    #[test]
    fn parse_valid_no_normalizing_required() {
        let parsed = Email::parse("someone@somewhere").expect("Parse failed");

        assert_eq!("someone@somewhere", parsed.provided_address());
        assert_eq!("someone@somewhere", parsed.normalized_address());
    }

    #[test]
    fn parse_valid_local_part_is_not_changed() {
        let parsed = Email::parse("TeSt@example.com").expect("Parse failed");

        assert_eq!("TeSt@example.com", parsed.provided_address());
        assert_eq!("TeSt@example.com", parsed.normalized_address());
    }

    #[test]
    fn parse_valid_normalize_domain() {
        let parsed = Email::parse("test@ExAmPlE.com").expect("Parse failed");

        assert_eq!("test@ExAmPlE.com", parsed.provided_address());
        assert_eq!("test@example.com", parsed.normalized_address());
    }
}
