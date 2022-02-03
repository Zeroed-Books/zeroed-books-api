use anyhow::Result;
use tera::Tera;

use crate::email::clients::EmailClient;

use super::domain::password_resets::NewPasswordReset;

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
