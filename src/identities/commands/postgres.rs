use anyhow::Result;
use tera::{Context, Tera};
use tracing::info;
use uuid::Uuid;

use crate::{
    email::clients::{EmailClient, Message},
    identities::{
        domain,
        models::password_resets::{NewPasswordReset, PasswordReset as PasswordResetModel},
    },
    PostgresConn,
};

use super::PasswordResetCommands;

pub struct PostgresCommands<'a>(pub &'a PostgresConn);

#[async_trait]
impl<'a> PasswordResetCommands for PostgresCommands<'a> {
    async fn create_reset_token(
        &self,
        password_reset: domain::password_resets::PasswordReset,
        mailer: &dyn EmailClient,
        tera: &Tera,
    ) -> Result<()> {
        let target_email = password_reset.email().address().to_owned();
        let saved_reset = self
            .0
            .run::<_, Result<_>>(move |conn| {
                use crate::schema;
                use diesel::prelude::*;

                let email_owner = schema::email::table
                    .filter(
                        schema::email::provided_address
                            .eq(password_reset.email().address())
                            .and(schema::email::verified_at.is_not_null()),
                    )
                    .select(schema::email::user_id);
                let owner_result = schema::user::table
                    .filter(schema::user::id.eq_any(email_owner))
                    .select(schema::user::id)
                    .get_result::<Uuid>(conn)
                    .optional()?;

                match owner_result {
                    None => Ok(None),
                    Some(owner) => {
                        let model = NewPasswordReset {
                            user_id: owner,
                            token: password_reset.token().to_owned(),
                        };

                        let saved_reset: PasswordResetModel =
                            diesel::insert_into(schema::password_resets::table)
                                .values(model)
                                .get_result(conn)?;

                        Ok(Some(saved_reset))
                    }
                }
            })
            .await?;

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
