use anyhow::Result;
use async_trait::async_trait;
use tera::Tera;

use crate::{email::clients::EmailClient, passwords::Password};

use super::domain::password_resets::{NewPasswordReset, PasswordResetToken};

pub mod postgres;

#[async_trait]
pub trait PasswordResetCommands {
    async fn create_reset_token(
        &self,
        password_reset: NewPasswordReset,
        mailer: &dyn EmailClient,
        tera: &Tera,
    ) -> Result<()>;
}

#[async_trait]
pub trait UserCommands {
    async fn reset_user_password(
        &self,
        token: PasswordResetToken,
        password: Password,
    ) -> Result<()>;
}
