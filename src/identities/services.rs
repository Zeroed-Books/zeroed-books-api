use std::{convert::TryFrom, sync::Arc};

use anyhow::Context;
use semval::ValidatedFrom;
use sqlx::PgPool;
use tera::Tera;
use tracing::{debug, error};

use crate::{
    email::clients::{EmailClient, Message},
    models::{self, NewUserModel},
    rate_limit::{RateLimitResult, RateLimiter},
};

use super::{
    domain::{
        email::EmailVerification,
        users::{NewUser, NewUserData, NewUserInvalidity},
    },
    models::email::{EmailPersistanceError, NewEmail, NewEmailVerification},
};

pub type DynEmailClient = Arc<dyn EmailClient>;
pub type DynRateLimiter = Arc<dyn RateLimiter>;

/// A service object providing functionality relating to users.
#[derive(Clone)]
pub struct UserService {
    db: PgPool,
    email_client: DynEmailClient,
    rate_limiter: DynRateLimiter,
    templates: Tera,
}

pub enum CreateUserResult {
    /// The new user was successfully created.
    ///
    /// This means that either the user did not already exist in the database
    /// and has now been persisted, or the provided email was a duplicate and
    /// a notice has been sent to the address.
    Created(NewUser),

    /// The provided user data is invalid.
    InvalidUser(semval::context::Context<NewUserInvalidity>),

    /// The operation is rate limited for the provided client.
    RateLimited(RateLimitResult),
}

impl UserService {
    /// Create a new user service.
    ///
    /// # Arguments
    ///
    /// * `db` - The database executor to use.
    /// * `email_client` - The client used to send emails.
    /// * `rate_limiter` - The rate limiter to use for rate limited operations.
    /// * `templates` - The templating engine to use for composing email
    ///   content.
    ///
    /// # Returns
    ///
    /// A new [`UserService`] instance.
    pub fn new(
        db: PgPool,
        email_client: DynEmailClient,
        rate_limiter: DynRateLimiter,
        templates: Tera,
    ) -> Self {
        Self {
            db,
            email_client,
            rate_limiter,
            templates,
        }
    }

    /// Create a new user.
    ///
    /// We only create a new user if the provided email does not match one that
    /// already exists. In the case of a duplicate email, we send a notification
    /// to that email that the account already exists. For new users, we persist
    /// the user and email, and then send a verification email.
    ///
    /// In either case, we don't return any information indicating whether or
    /// not the email already existed in order to avoid leaking information.
    ///
    /// # Arguments
    ///
    /// * `client_identifier` - A unique identifier for the client performing
    ///   the operation. This is used for rate limiting.
    /// * `new_user_data` - The new user's information.
    pub async fn create_user(
        &self,
        client_identifier: &str,
        new_user_data: NewUserData,
    ) -> anyhow::Result<CreateUserResult> {
        let rate_limit_key = format!("/identities/users_post_{}", client_identifier);
        match self.rate_limiter.is_limited(&rate_limit_key, 10) {
            Ok(RateLimitResult::NotLimited) => (),
            Ok(result @ RateLimitResult::LimitedUntil(_)) => {
                return Ok(CreateUserResult::RateLimited(result));
            }
            Err(error) => {
                error!(?error, "Failed to query rate limiter for creating user.");

                return Err(error);
            }
        };

        let new_user = match NewUser::validated_from(new_user_data) {
            Ok(user) => user,
            Err((_, context)) => {
                debug!(?context, "New user data is invalid.");

                return Ok(CreateUserResult::InvalidUser(context));
            }
        };

        let user_model = models::NewUserModel::try_from(&new_user)
            .context("Failed to convert from domain to model.")?;
        let email_model = NewEmail::for_user(new_user.id(), new_user.email());

        let persistance_result = self.persist_new_user(&user_model, &email_model).await;

        if let Err(persistence_err) = persistance_result {
            match persistence_err {
                EmailPersistanceError::DuplicateEmail(_) => {
                    let content = self
                        .templates
                        .render("emails/duplicate.txt", &tera::Context::new())?;

                    let message = Message {
                        to: new_user.email().address().to_owned(),
                        subject: "Duplicate Registration".to_owned(),
                        text: content,
                    };

                    self.email_client
                        .send(&message)
                        .await
                        .context("Failed to send duplicate registration email.")?;

                    return Ok(CreateUserResult::Created(new_user));
                }
                error => {
                    error!(?error, "Failed to persist new user.");

                    return Err(error.into());
                }
            }
        }

        let verification = EmailVerification::new();
        let verification_model = NewEmailVerification::new(email_model.id(), &verification);

        verification_model
            .save(&self.db)
            .await
            .context("Failed to save email verification model.")?;

        let mut verification_context = tera::Context::new();
        verification_context.insert("token", verification.token());

        let content = self
            .templates
            .render("emails/verify.txt", &verification_context)
            .context("Failed to render email verification template.")?;

        let message = Message {
            to: new_user.email().address().to_owned(),
            subject: "Please Confirm your Email".to_owned(),
            text: content,
        };

        self.email_client
            .send(&message)
            .await
            .context("Failed to send verification email.")?;

        Ok(CreateUserResult::Created(new_user))
    }

    async fn persist_new_user(
        &self,
        user: &NewUserModel,
        email: &NewEmail,
    ) -> Result<(), EmailPersistanceError> {
        let mut tx = self.db.begin().await?;

        sqlx::query!(
            r#"
            INSERT INTO "user" (id, password)
            VALUES ($1, $2)
            "#,
            user.id,
            user.password_hash
        )
        .execute(&mut tx)
        .await?;

        email.save(&mut tx).await?;

        tx.commit().await?;

        Ok(())
    }
}
