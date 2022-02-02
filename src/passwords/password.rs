use std::fmt::Debug;

use semval::prelude::*;

const MAX_PASSWORD_LENGTH: usize = 512;
const MIN_PASSWORD_LENGTH: usize = 8;

/// A user's password.
pub struct Password(String);

impl Password {
    /// Construct an unvalidated password.
    ///
    /// This can be useful when constructing an object that contains a password
    /// so that the object can be validated as a whole.
    ///
    /// # Arguments
    ///
    /// * `password` - The password to store.
    pub fn unvalidated(password: String) -> Self {
        Self(password)
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PasswordInvalidity {
    /// The provided value exceeds the maximum allowable length for a password.
    /// The max length is contained as a value.
    MaxLength(usize),
    /// The provided value is smaller than the minimum allowable length for a
    /// password. The min length is contained as a value.
    MinLength(usize),
}

impl Validate for Password {
    type Invalidity = PasswordInvalidity;

    fn validate(&self) -> ValidationResult<Self::Invalidity> {
        ValidationContext::new()
            .invalidate_if(
                self.0.len() < MIN_PASSWORD_LENGTH,
                PasswordInvalidity::MinLength(MIN_PASSWORD_LENGTH),
            )
            .invalidate_if(
                self.0.len() > MAX_PASSWORD_LENGTH,
                PasswordInvalidity::MaxLength(MAX_PASSWORD_LENGTH),
            )
            .into()
    }
}

impl ValidatedFrom<&str> for Password {
    fn validated_from(from: &str) -> ValidatedResult<Self> {
        let into = Password(from.to_owned());

        match into.validate() {
            Ok(_) => Ok(into),
            Err(context) => Err((into, context)),
        }
    }
}

impl Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Don't include the raw password in debug output.
        f.debug_tuple("Password").field(&"*".repeat(8)).finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn debug_does_not_contain_value() {
        let raw_password = "some-very-unique-string";
        let password = Password::validated_from(raw_password).expect("Password should be valid");

        let debug_output = format!("{:?}", password);

        assert!(
            !debug_output.contains(raw_password),
            "The raw password {:?} should not be contained in the debug output {:?}.",
            raw_password,
            password
        );
    }

    #[test]
    fn validate_from_valid() {
        let raw_password = "password";

        let password = Password::validated_from(raw_password).expect("Password should be valid");

        assert_eq!(raw_password, &password.0);
    }

    #[test]
    fn validate_from_too_short() {
        let raw_password = "a".repeat(7);

        let (_, context) = Password::validated_from(raw_password.as_ref())
            .expect_err("Password should be too short");

        let invalidities = context.into_iter().collect::<Vec<_>>();

        assert_eq!(1, invalidities.len());
        assert_eq!(PasswordInvalidity::MinLength(8), invalidities[0]);
    }

    #[test]
    fn validate_from_too_long() {
        let raw_password = "a".repeat(513);

        let (_, context) = Password::validated_from(raw_password.as_ref())
            .expect_err("Password should be too long");

        let invalidities = context.into_iter().collect::<Vec<_>>();

        assert_eq!(1, invalidities.len());
        assert_eq!(PasswordInvalidity::MaxLength(512), invalidities[0]);
    }
}
