use rand::{distributions::Alphanumeric, thread_rng, Rng};
use semval::prelude::*;

use super::email::{Email, EmailInvalidity};

#[derive(Debug)]
pub struct PasswordReset {
    email: Email,
    token: String,
}

const RESET_TOKEN_LENGTH: usize = 64;
impl PasswordReset {
    /// Create a new password reset.
    ///
    /// # Arguments
    ///
    /// * `email` - The email address of the user requesting a password reset.
    ///
    /// # Returns
    ///
    /// A new password reset for the provided email address with a randomly
    /// generated token.
    pub fn new(email: Email) -> Self {
        let token: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(RESET_TOKEN_LENGTH)
            .map(char::from)
            .collect();

        Self { email, token }
    }

    pub fn email(&self) -> &Email {
        &self.email
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}

impl Validate for PasswordReset {
    type Invalidity = EmailInvalidity;

    fn validate(&self) -> ValidationResult<Self::Invalidity> {
        ValidationContext::new().validate(&self.email).into()
    }
}

impl ValidatedFrom<&str> for PasswordReset {
    fn validated_from(from: &str) -> ValidatedResult<Self> {
        let into = Self::new(Email::unvalidated(from.to_owned()));

        match into.validate() {
            Ok(()) => Ok(into),
            Err(context) => Err((into, context)),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn validated_from_invalid_email() {
        let (_, context) = PasswordReset::validated_from("some-invalid-email")
            .expect_err("invalid email should not validate");
        let errors = context.into_iter().collect::<Vec<_>>();

        assert!(!errors.is_empty());
    }

    #[test]
    fn validated_from_valid_email() {
        let reset =
            PasswordReset::validated_from("test@example.com").expect("valid email should validate");

        assert_eq!(RESET_TOKEN_LENGTH, reset.token().len());
    }
}
