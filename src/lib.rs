#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate rocket;

use std::net::IpAddr;

use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use diesel::{insert_into, RunQueryDsl};
use email::clients::{EmailClient, Message};
use http_err::ApiError;
use identities::{
    domain::email::{Email, EmailVerification},
    models::email::{EmailPersistanceError, NewEmail, NewEmailVerification},
};
use models::NewUser;
use rand_core::OsRng;
use rate_limit::{RateLimitResult, RateLimiter};
use rocket::{
    response::Responder,
    serde::{json::Json, Deserialize, Serialize},
    State,
};
use rocket_sync_db_pools::database;
use tera::{Context, Tera};
use uuid::Uuid;

use crate::http_err::InternalServerError;

pub mod cli;
mod email;
mod http_err;
mod identities;
mod models;
mod rate_limit;
mod schema;
mod server;

#[database("postgres")]
pub struct PostgresConn(diesel::PgConnection);

#[derive(Deserialize)]
pub struct NewUserRequest<'r> {
    email: &'r str,
    password: &'r str,
}

#[derive(Serialize)]
pub struct NewUserResponse {
    email: String,
}

#[derive(Responder)]
pub enum CreateUserResponse {
    #[response(status = 201)]
    UserCreated(Json<NewUserResponse>),
}

#[post("/users", data = "<new_user>")]
pub async fn create_user(
    db: PostgresConn,
    // TODO: Handle client IPs behind proxy.
    client_ip: IpAddr,
    email_client: &State<Box<dyn EmailClient>>,
    rate_limiter: &State<Box<dyn RateLimiter>>,
    templates: &State<Tera>,
    new_user: Json<NewUserRequest<'_>>,
) -> Result<CreateUserResponse, ApiError> {
    let rate_limit_key = format!("/users_post_{}", client_ip.to_string());
    match rate_limiter.is_limited(&rate_limit_key, 10) {
        Ok(RateLimitResult::NotLimited) => (),
        Ok(result @ RateLimitResult::LimitedUntil(_)) => return Err(result.into()),
        Err(err) => {
            eprintln!("{:?}", err);
            return Err(InternalServerError {
                message: "Internal server error.".to_string(),
            }
            .into());
        }
    };

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let password_hash = argon2
        .hash_password_simple(new_user.password.as_bytes(), salt.as_ref())
        .unwrap()
        .to_string();

    let new_user_id = Uuid::new_v4();
    let user_model = NewUser {
        id: new_user_id,
        password_hash: password_hash,
    };

    let email = match Email::parse(new_user.email) {
        Ok(parsed) => parsed,
        Err(_) => {
            return Err(InternalServerError {
                message: "This should be a form error: Invalid email address.".to_owned(),
            }
            .into());
        }
    };

    let email_model = NewEmail::for_user(new_user_id, &email);
    let new_email_id = email_model.id();

    let persistance_result = db
        .run(|c| persist_new_user(c, user_model, email_model))
        .await;

    if let Err(persistence_err) = persistance_result {
        match persistence_err {
            EmailPersistanceError::DuplicateEmail(_) => {
                let content = templates
                    .render("emails/duplicate.txt", &Context::new())
                    .expect("template failure");

                let message = Message {
                    // TODO: Pull from environment.
                    from: "no-reply@zeroedbooks.com".to_owned(),
                    to: email.provided_address().to_string(),
                    subject: "Duplicate Registration".to_owned(),
                    text: content,
                };

                match email_client.send(&message).await {
                    Ok(()) => {
                        return Ok(CreateUserResponse::UserCreated(Json(NewUserResponse {
                            email: email.provided_address().to_string(),
                        })))
                    }
                    Err(()) => {
                        // TODO: Logging.
                        return Err(InternalServerError {
                            message: "Internal server error.".to_owned(),
                        }
                        .into());
                    }
                }
            }
            _ => {
                // TODO: Logging.
                return Err(InternalServerError {
                    message: "Internal server error.".to_owned(),
                }
                .into());
            }
        }
    }

    let verification = EmailVerification::new();
    let verification_model = NewEmailVerification::new(new_email_id, &verification);

    let verification_save_result = db.run(move |conn| verification_model.save(conn)).await;
    match verification_save_result {
        Ok(()) => (),
        Err(_) => {
            // TODO: Logging.
            return Err(InternalServerError {
                message: "Internal server error.".to_owned(),
            }
            .into());
        }
    };

    let mut verification_context = Context::new();
    verification_context.insert("token", verification.token());

    let content = templates
        .render("emails/verify.txt", &verification_context)
        .expect("template failure");

    let message = Message {
        // TODO: Pull from environment.
        from: "no-reply@zeroedbooks.com".to_owned(),
        to: email.provided_address().to_string(),
        subject: "Please Confirm your Email".to_owned(),
        text: content,
    };

    match email_client.send(&message).await {
        Ok(()) => (),
        Err(()) => {
            // TODO: Log
            return Err(InternalServerError {
                message: "Internal server error.".to_owned(),
            }
            .into());
        }
    }

    Ok(CreateUserResponse::UserCreated(Json(NewUserResponse {
        email: email.provided_address().to_string(),
    })))
}

fn persist_new_user<'a>(
    conn: &diesel::PgConnection,
    user: NewUser,
    email: NewEmail,
) -> Result<(), EmailPersistanceError> {
    use crate::schema::user;

    conn.build_transaction().run(|| {
        insert_into(user::table).values(&user).execute(conn)?;

        email.save(conn)
    })
}
