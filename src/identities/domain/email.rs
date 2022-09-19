use rand::{distributions::Alphanumeric, thread_rng, Rng};
use semval::prelude::*;

#[derive(Debug, Eq, PartialEq)]
pub struct Email(String);

impl Email {
    /// Create an unvalidated email.
    ///
    /// This can be useful when constructing an object that contains an email
    /// but has not been validated yet.
    ///
    /// # Arguments
    ///
    /// * `address` - The email's address.
    pub fn unvalidated(address: String) -> Self {
        Self(address)
    }

    pub fn address(&self) -> &str {
        &self.0
    }

    fn has_domain(&self) -> bool {
        if let Some(index) = self.0.find('@') {
            index < self.0.len() - 1
        } else {
            false
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum EmailInvalidity {
    /// The address does not have a domain portion.
    MissingDomain,

    /// The address is missing the `@` symbol separating the local and domain
    /// parts.
    MissingSeparator,
}

impl Validate for Email {
    type Invalidity = EmailInvalidity;

    fn validate(&self) -> ValidationResult<Self::Invalidity> {
        ValidationContext::new()
            .invalidate_if(!self.0.contains('@'), EmailInvalidity::MissingSeparator)
            .invalidate_if(!self.has_domain(), EmailInvalidity::MissingDomain)
            .into()
    }
}

impl ValidatedFrom<&str> for Email {
    fn validated_from(from: &str) -> ValidatedResult<Self> {
        let into = Self(from.to_owned());

        match into.validate() {
            Ok(()) => Ok(into),
            Err(context) => Err((into, context)),
        }
    }
}

const VERIFICATION_TOKEN_LENGTH: usize = 64;

pub struct EmailVerification {
    token: String,
}

impl EmailVerification {
    pub fn new() -> Self {
        let token: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(VERIFICATION_TOKEN_LENGTH)
            .map(char::from)
            .collect();

        Self { token }
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn validated_from_missing_at_symbol() {
        let (_, context) = Email::validated_from("missing-an-at-symbol").expect_err("missing an @");
        let errors = context.into_iter().collect::<Vec<_>>();

        assert_eq!(2, errors.len());
        assert_eq!(EmailInvalidity::MissingSeparator, errors[0]);
    }

    #[test]
    fn validated_from_valid_missing_domain() {
        let (_, context) = Email::validated_from("someone@").expect_err("missing a domain");
        let errors = context.into_iter().collect::<Vec<_>>();

        assert_eq!(1, errors.len());
        assert_eq!(EmailInvalidity::MissingDomain, errors[0]);
    }

    #[test]
    fn validated_from_valid() {
        let parsed = Email::validated_from("someone@somewhere").expect("Parse failed");

        assert_eq!("someone@somewhere", parsed.address());
    }
}
