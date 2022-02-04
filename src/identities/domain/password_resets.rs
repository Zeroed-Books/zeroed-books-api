use std::fmt::Debug;

use chrono::{DateTime, Duration, Utc};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use semval::prelude::*;
use uuid::Uuid;

use super::email::{Email, EmailInvalidity};

#[derive(Debug)]
pub struct NewPasswordReset {
    email: Email,
    token: String,
}

const RESET_TOKEN_LENGTH: usize = 64;
impl NewPasswordReset {
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

impl Validate for NewPasswordReset {
    type Invalidity = EmailInvalidity;

    fn validate(&self) -> ValidationResult<Self::Invalidity> {
        ValidationContext::new().validate(&self.email).into()
    }
}

impl ValidatedFrom<&str> for NewPasswordReset {
    fn validated_from(from: &str) -> ValidatedResult<Self> {
        let into = Self::new(Email::unvalidated(from.to_owned()));

        match into.validate() {
            Ok(()) => Ok(into),
            Err(context) => Err((into, context)),
        }
    }
}

pub struct PasswordResetToken {
    user_id: Uuid,
    token: String,
    created_at: DateTime<Utc>,
}

impl PasswordResetToken {
    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    fn is_expired(&self) -> bool {
        let expiration_date = self.created_at + Duration::hours(1);

        Utc::now() > expiration_date
    }
}

impl Debug for PasswordResetToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PasswordResetToken")
            .field("user_id", &self.user_id)
            .field("token", &"*".repeat(8))
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum PasswordResetTokenInvalidity {
    Expired,
}

impl Validate for PasswordResetToken {
    type Invalidity = PasswordResetTokenInvalidity;

    fn validate(&self) -> ValidationResult<Self::Invalidity> {
        ValidationContext::new()
            .invalidate_if(self.is_expired(), Self::Invalidity::Expired)
            .into()
    }
}

pub struct PasswordResetTokenData {
    pub user_id: Uuid,
    pub token: String,
    pub created_at: DateTime<Utc>,
}

impl ValidatedFrom<PasswordResetTokenData> for PasswordResetToken {
    fn validated_from(from: PasswordResetTokenData) -> ValidatedResult<Self> {
        let into = PasswordResetToken {
            user_id: from.user_id,
            token: from.token,
            created_at: from.created_at,
        };

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
    fn new_password_reset_validated_from_invalid_email() {
        let (_, context) = NewPasswordReset::validated_from("some-invalid-email")
            .expect_err("invalid email should not validate");
        let errors = context.into_iter().collect::<Vec<_>>();

        assert!(!errors.is_empty());
    }

    #[test]
    fn new_password_reset_validated_from_valid_email() {
        let reset = NewPasswordReset::validated_from("test@example.com")
            .expect("valid email should validate");

        assert_eq!(RESET_TOKEN_LENGTH, reset.token().len());
    }

    #[test]
    fn password_reset_token_validate_valid() {
        let reset = PasswordResetToken {
            user_id: Uuid::new_v4(),
            token: "some-token".to_owned(),
            created_at: Utc::now(),
        };

        assert!(reset.validate().is_ok());
    }

    #[test]
    fn password_reset_token_validate_expired() {
        let reset = PasswordResetToken {
            user_id: Uuid::new_v4(),
            token: "expired-token".to_owned(),
            // Tokens should be valid for one hour, so ours should be expired by
            // one second.
            created_at: Utc::now() - Duration::seconds(60 * 60 + 1),
        };

        let context = reset.validate().expect_err("should be expired");
        let errors = context.into_iter().collect::<Vec<_>>();

        assert_eq!(1, errors.len());
        assert_eq!(PasswordResetTokenInvalidity::Expired, errors[0]);
    }
}
