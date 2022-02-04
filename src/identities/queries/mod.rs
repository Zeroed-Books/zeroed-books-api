use anyhow::Result;

use super::domain::password_resets::PasswordResetTokenData;

pub mod postgres;

#[derive(Debug)]
pub enum PasswordResetError {
    NotFound,
    Unknown(anyhow::Error),
}

#[async_trait]
pub trait PasswordResetQueries {
    async fn get_password_reset(
        &self,
        token: String,
    ) -> Result<PasswordResetTokenData, PasswordResetError>;
}
