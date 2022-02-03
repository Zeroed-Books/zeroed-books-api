use semval::context::Context as ValidationContext;
use serde::{Deserialize, Serialize};

use crate::{
    identities::domain::{self, email::EmailInvalidity, users::NewUserInvalidity},
    passwords::PasswordInvalidity,
};

#[derive(Deserialize)]
pub struct NewUserRequest<'r> {
    email: &'r str,
    password: &'r str,
}

impl<'r> From<NewUserRequest<'r>> for domain::users::NewUserData<'r> {
    fn from(rep: NewUserRequest<'r>) -> Self {
        Self {
            email: rep.email,
            password: rep.password,
        }
    }
}

#[derive(Serialize)]
pub struct NewUserResponse {
    pub email: String,
}

#[derive(Default, Serialize)]
pub struct NewUserValidationError {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    email: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    password: Vec<String>,
}

impl From<ValidationContext<NewUserInvalidity>> for NewUserValidationError {
    fn from(validation: ValidationContext<NewUserInvalidity>) -> Self {
        let mut response = NewUserValidationError::default();

        for invalidity in validation.into_iter() {
            match invalidity {
                NewUserInvalidity::Email(email_invalidity) => match email_invalidity {
                    EmailInvalidity::MissingDomain => {
                        response.email.push("Email is missing a domain.".to_owned())
                    }
                    EmailInvalidity::MissingSeparator => response
                        .email
                        .push("Email is missing an '@' symbol.".to_owned()),
                },
                NewUserInvalidity::Password(password_invalidity) => match password_invalidity {
                    PasswordInvalidity::MaxLength(max) => response.password.push(format!(
                        "Passwords may not contain more than {} characters.",
                        max
                    )),
                    PasswordInvalidity::MinLength(min) => response.password.push(format!(
                        "Passwords must contain at least {} characters.",
                        min
                    )),
                },
            }
        }

        response
    }
}

#[derive(Deserialize, Serialize)]
pub struct PasswordResetRequest<'r> {
    pub email: &'r str,
}

#[derive(Default, Serialize)]
pub struct PasswordResetError {
    email: Vec<String>,
}

impl From<ValidationContext<EmailInvalidity>> for PasswordResetError {
    fn from(validation: ValidationContext<EmailInvalidity>) -> Self {
        let mut response = PasswordResetError::default();

        for invalidity in validation.into_iter() {
            match invalidity {
                EmailInvalidity::MissingDomain => {
                    response.email.push("Email is missing a domain.".to_owned())
                }
                EmailInvalidity::MissingSeparator => response
                    .email
                    .push("Email is missing an '@' symbol.".to_owned()),
            }
        }

        response
    }
}
