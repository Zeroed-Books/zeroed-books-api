use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use tera::{Context, Tera};
use tracing::{debug, info};

use crate::{
    email::clients::{EmailClient, Message},
    identities::{domain, models::password_resets::PasswordReset as PasswordResetModel},
    passwords::{self, Password},
};

use super::{PasswordResetCommands, UserCommands};

pub struct PostgresCommands<'a>(pub &'a PgPool);

#[async_trait]
impl<'a> PasswordResetCommands for PostgresCommands<'a> {
    async fn create_reset_token(
        &self,
        password_reset: domain::password_resets::NewPasswordReset,
        mailer: &dyn EmailClient,
        tera: &Tera,
    ) -> Result<()> {
        let target_email = password_reset.email().address().to_owned();
        let owner_result = sqlx::query!(
            r#"
            SELECT e.user_id
            FROM "email" e
            WHERE e.provided_address = $1 AND e.verified_at IS NOT NULL
            "#,
            password_reset.email().address()
        )
        .fetch_optional(self.0)
        .await?;

        let saved_reset = match owner_result {
            None => None,
            Some(owner) => {
                let saved_reset = sqlx::query_as!(
                    PasswordResetModel,
                    r#"
                    INSERT INTO "password_resets" (token, user_id)
                    VALUES ($1, $2)
                    RETURNING token, user_id, created_at
                    "#,
                    password_reset.token(),
                    owner.user_id,
                )
                .fetch_one(self.0)
                .await?;

                Some(saved_reset)
            }
        };

        match saved_reset {
            Some(reset) => {
                let mut context = Context::new();
                context.insert("token", &reset.token);

                let content = tera.render("emails/reset_password_token.txt", &context)?;
                let message = Message {
                    to: target_email,
                    subject: "Reset Your Password".to_owned(),
                    text: content,
                };

                mailer.send(&message).await?;

                info!(user_id = %reset.user_id, "Sent password reset token to verified email.");
            }
            None => {
                let content =
                    tera.render("emails/reset_password_no_account.txt", &Context::new())?;
                let message = Message {
                    to: target_email,
                    subject: "Password Reset Attempt".to_owned(),
                    text: content,
                };

                mailer.send(&message).await?;

                info!("Sent password reset attempt notification to unverified email.");
            }
        }

        Ok(())
    }
}

#[async_trait]
impl<'a> UserCommands for PostgresCommands<'a> {
    async fn reset_user_password(
        &self,
        reset_token: domain::password_resets::PasswordResetToken,
        password: Password,
    ) -> Result<()> {
        let user_id = reset_token.user_id();
        let hash = passwords::Hash::new(&password)?;

        sqlx::query!(
            r#"
            UPDATE "user"
            SET password = $2
            WHERE id = $1
            "#,
            user_id,
            hash.value()
        )
        .execute(self.0)
        .await?;

        debug!(%user_id, "Changed user's password using reset token.");

        sqlx::query!(
            r#"
                    DELETE FROM password_resets
                    WHERE token = $1
                    "#,
            reset_token.token()
        )
        .execute(self.0)
        .await?;

        debug!(%user_id, "Deleted password reset token.");
        info!(%user_id, "Reset user's password.");

        Ok(())
    }
}
