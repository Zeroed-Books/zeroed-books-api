use anyhow::Result;

use crate::{
    identities::{
        domain::password_resets::PasswordResetTokenData, models::password_resets::PasswordReset,
    },
    PostgresConn,
};

use super::{PasswordResetError, PasswordResetQueries};

pub struct PostgresQueries<'a>(pub &'a PostgresConn);

impl From<diesel::result::Error> for PasswordResetError {
    fn from(error: diesel::result::Error) -> Self {
        Self::Unknown(error.into())
    }
}

#[async_trait]
impl<'a> PasswordResetQueries for PostgresQueries<'a> {
    async fn get_password_reset(
        &self,
        provided_token: String,
    ) -> Result<PasswordResetTokenData, PasswordResetError> {
        let reset = self
            .0
            .run::<_, Result<_, PasswordResetError>>(move |conn| {
                use crate::schema::password_resets::dsl::*;
                use diesel::prelude::*;

                Ok(password_resets
                    .filter(token.eq(provided_token))
                    .get_result::<PasswordReset>(conn)
                    .optional()?)
            })
            .await?;

        match reset {
            Some(reset) => Ok(reset.into()),
            None => Err(PasswordResetError::NotFound),
        }
    }
}
