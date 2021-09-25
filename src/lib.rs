#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate rocket;

use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use diesel::{insert_into, RunQueryDsl};
use email::clients::{EmailClient, Message};
use identities::{
    domain::email::Email,
    models::email::{EmailPersistanceError, NewEmail},
};
use models::NewUser;
use rand_core::OsRng;
use rocket::{
    http::{ContentType, Status},
    response::Responder,
    serde::{json::Json, Deserialize, Serialize},
    Response, State,
};
use rocket_sync_db_pools::database;
use tera::{Context, Tera};
use uuid::Uuid;

pub mod cli;
mod email;
mod identities;
mod models;
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

#[derive(Serialize)]
pub struct GenericError {
    pub message: String,
}

pub struct ApiResponse<T: Serialize> {
    pub value: Json<T>,
    pub status: Status,
}

impl<'r, 'o: 'r, T: Serialize> Responder<'r, 'o> for ApiResponse<T> {
    fn respond_to(self, request: &'r rocket::Request<'_>) -> rocket::response::Result<'o> {
        Response::build_from(self.value.respond_to(request)?)
            .status(self.status)
            .header(ContentType::JSON)
            .ok()
    }
}

#[post("/users", data = "<new_user>")]
pub async fn create_user(
    db: PostgresConn,
    email_client: &State<Box<dyn EmailClient>>,
    templates: &State<Tera>,
    new_user: Json<NewUserRequest<'_>>,
) -> Result<Json<NewUserResponse>, ApiResponse<GenericError>> {
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
            return Err(ApiResponse {
                value: Json(GenericError {
                    message: "Invalid email address.".to_owned(),
                }),
                status: Status::BadRequest,
            })
        }
    };

    let email_model = NewEmail::for_user(new_user_id, &email);

    let persistance_result = db
        .run(|c| persist_new_user(c, user_model, email_model))
        .await;

    match persistance_result {
        Ok(_) => {
            let content = templates
                .render("emails/verify.txt", &Context::new())
                .expect("template failure");

            let message = Message {
                // TODO: Pull from environment.
                from: "no-reply@zeroedbooks.com".to_owned(),
                to: email.provided_address().to_string(),
                subject: "Please Confirm your Email".to_owned(),
                // TODO: Generate email verification codes
                text: content,
            };

            match email_client.send(&message).await {
                Ok(()) => (),
                Err(()) => {
                    // TODO: Log
                    return Err(ApiResponse {
                        value: Json(GenericError {
                            message: "Internal server error.".to_owned(),
                        }),
                        status: Status::InternalServerError,
                    });
                }
            }
        }
        Err(EmailPersistanceError::DuplicateEmail(_)) => {
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
                Ok(()) => (),
                Err(()) => {
                    // TODO: Logging.
                    return Err(ApiResponse {
                        value: Json(GenericError {
                            message: "Internal server error.".to_owned(),
                        }),
                        status: Status::InternalServerError,
                    });
                }
            }
        }
        Err(_) => {
            // TODO: Logging.
            return Err(ApiResponse {
                value: Json(GenericError {
                    message: "Internal server error.".to_owned(),
                }),
                status: Status::InternalServerError,
            });
        }
    };

    Ok(Json(NewUserResponse {
        email: email.provided_address().to_string(),
    }))
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
