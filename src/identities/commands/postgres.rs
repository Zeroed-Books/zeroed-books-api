use anyhow::Result;
use tera::{Context, Tera};
use tracing::{debug, info};
use uuid::Uuid;

use crate::{
    email::clients::{EmailClient, Message},
    identities::{
        domain,
        models::password_resets::{NewPasswordReset, PasswordReset as PasswordResetModel},
    },
    passwords::{self, Password},
    PostgresConn,
};

use super::{PasswordResetCommands, UserCommands};

pub struct PostgresCommands<'a>(pub &'a PostgresConn);

#[async_trait]
impl<'a> PasswordResetCommands for PostgresCommands<'a> {
    async fn create_reset_token(
        &self,
        password_reset: domain::password_resets::NewPasswordReset,
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

#[async_trait]
impl<'a> UserCommands for PostgresCommands<'a> {
    async fn reset_user_password(
        &self,
        reset_token: domain::password_resets::PasswordResetToken,
        password: Password,
    ) -> Result<()> {
        let user_id = reset_token.user_id();
        let hash = passwords::Hash::new(&password)?;

        self.0
            .run(move |conn| {
                use diesel::prelude::*;

                {
                    use crate::schema::user::dsl::*;

                    diesel::update(user)
                        .set(password.eq(hash.value()))
                        .filter(id.eq(user_id))
                        .execute(conn)?;
                }

                debug!(%user_id, "Changed user's password using reset token.");

                {
                    use crate::schema::password_resets::dsl::*;

                    diesel::delete(password_resets)
                        .filter(token.eq(reset_token.token()))
                        .execute(conn)
                }
            })
            .await?;

        debug!(%user_id, "Deleted password reset token.");
        info!(%user_id, "Reset user's password.");

        Ok(())
    }
}
