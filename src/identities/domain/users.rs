use anyhow::Result;
use semval::prelude::*;
use uuid::Uuid;

use crate::passwords::{self, Password, PasswordInvalidity};

use super::email::{Email, EmailInvalidity};

#[derive(Debug)]
pub struct NewUser {
    id: Uuid,
    email: Email,
    password: Password,
}

impl NewUser {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn email(&self) -> &Email {
        &self.email
    }

    pub fn password_hash(&self) -> Result<passwords::Hash> {
        passwords::Hash::new(&self.password)
    }
}

#[derive(Debug)]
pub enum NewUserInvalidity {
    Email(EmailInvalidity),
    Password(PasswordInvalidity),
}

impl Validate for NewUser {
    type Invalidity = NewUserInvalidity;

    fn validate(&self) -> ValidationResult<Self::Invalidity> {
        ValidationContext::new()
            .validate_with(&self.email, NewUserInvalidity::Email)
            .validate_with(&self.password, NewUserInvalidity::Password)
            .into()
    }
}

#[derive(Clone, Debug)]
pub struct NewUserData {
    pub email: String,
    pub password: String,
}

impl ValidatedFrom<NewUserData> for NewUser {
    fn validated_from(from: NewUserData) -> ValidatedResult<Self> {
        let into = NewUser {
            id: Uuid::new_v4(),
            email: Email::unvalidated(from.email.to_owned()),
            password: Password::unvalidated(from.password),
        };

        match into.validate() {
            Ok(()) => Ok(into),
            Err(context) => Err((into, context)),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    pub fn validated_from_valid() -> Result<()> {
        let data = NewUserData {
            email: "test@example.com".to_owned(),
            password: "CorrectHorseBatteryStaple".to_owned(),
        };

        let new_user = NewUser::validated_from(data.clone()).expect("user should be valid");

        assert_eq!(data.email, new_user.email().address());
        assert!(new_user
            .password_hash()?
            .matches_raw_password(&data.password)?);

        Ok(())
    }
}
