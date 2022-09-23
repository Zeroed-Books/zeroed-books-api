use semval::context::Context as ValidationContext;
use serde::{Deserialize, Serialize};

use crate::{
    identities::{
        domain::{
            self, email::EmailInvalidity, password_resets::PasswordResetTokenInvalidity,
            users::NewUserInvalidity,
        },
        queries,
    },
    passwords::PasswordInvalidity,
};

#[derive(Deserialize)]
pub struct NewUserRequest {
    email: String,
    password: String,
}

impl From<NewUserRequest> for domain::users::NewUserData {
    fn from(rep: NewUserRequest) -> Self {
        Self {
            email: rep.email.to_owned(),
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
pub struct PasswordResetRequest {
    pub email: String,
}

#[derive(Default, Serialize)]
pub struct PasswordResetRequestError {
    email: Vec<String>,
}

impl From<ValidationContext<EmailInvalidity>> for PasswordResetRequestError {
    fn from(validation: ValidationContext<EmailInvalidity>) -> Self {
        let mut response = PasswordResetRequestError::default();

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

#[derive(Deserialize)]
pub struct PasswordReset {
    pub token: String,
    pub new_password: String,
}

#[derive(Default, Serialize)]
pub struct PasswordResetError {
    new_password: Vec<String>,
    token: Vec<String>,
}

impl From<ValidationContext<PasswordInvalidity>> for PasswordResetError {
    fn from(validation: ValidationContext<PasswordInvalidity>) -> Self {
        let mut response = Self::default();

        for invalidity in validation.into_iter() {
            match invalidity {
                PasswordInvalidity::MaxLength(max) => response.new_password.push(format!(
                    "Passwords may not contain more than {} characters.",
                    max
                )),
                PasswordInvalidity::MinLength(min) => response.new_password.push(format!(
                    "Passwords must contain at least {} characters.",
                    min
                )),
            }
        }

        response
    }
}

impl From<ValidationContext<PasswordResetTokenInvalidity>> for PasswordResetError {
    fn from(validation: ValidationContext<PasswordResetTokenInvalidity>) -> Self {
        let mut response = Self::default();

        if validation.into_iter().next().is_some() {
            response.token =
                vec!["The provided password reset token has expired or does not exist.".to_owned()];
        }

        response
    }
}

impl From<queries::PasswordResetError> for PasswordResetError {
    fn from(_error: queries::PasswordResetError) -> Self {
        Self {
            new_password: vec![],
            token: vec![
                "The provided password reset token has expired or does not exist.".to_owned(),
            ],
        }
    }
}
