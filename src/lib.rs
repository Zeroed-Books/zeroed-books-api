#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate rocket;

use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use diesel::{insert_into, RunQueryDsl};
use models::NewUser;
use rand_core::OsRng;
use rocket::{
    http::{ContentType, Status},
    response::Responder,
    serde::{json::Json, Deserialize, Serialize},
    Response,
};
use rocket_sync_db_pools::database;
use uuid::Uuid;

use crate::models::NewEmail;

pub mod cli;
pub mod models;
pub mod schema;
pub mod server;

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

    let email_model = NewEmail {
        provided_address: new_user.email.to_string(),
        // TODO: Normalize email
        normalized_address: new_user.email.to_string(),
        user_id: new_user_id,
    };

    let persistance_result = db
        .run(move |c| persist_new_user(c, user_model, email_model))
        .await;

    match persistance_result {
        Ok(()) => Ok(Json(NewUserResponse {
            email: new_user.email.to_string(),
        })),
        // TODO: Send emails instead of leaking registration info.
        Err(EmailPersistanceError::DuplicateEmail(_)) => Err(ApiResponse {
            value: Json(GenericError {
                message: "A user with that email already exists.".to_owned(),
            }),
            status: Status::BadRequest,
        }),
        Err(_) => Err(ApiResponse {
            value: Json(GenericError {
                message: "Internal server error.".to_owned(),
            }),
            status: Status::InternalServerError,
        }),
    }
}

fn persist_new_user(
    conn: &diesel::PgConnection,
    user: NewUser,
    email: NewEmail,
) -> Result<(), EmailPersistanceError> {
    use crate::schema::{email, user};
    use diesel::result::{DatabaseErrorKind, Error};

    conn.build_transaction().run(|| {
        insert_into(user::table).values(&user).execute(conn)?;

        match insert_into(email::table).values(&email).execute(conn) {
            Ok(_) => Ok(()),
            Err(Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => Err(
                EmailPersistanceError::DuplicateEmail(email.provided_address.to_string()),
            ),
            Err(err) => Err(EmailPersistanceError::DatabaseError(err)),
        }
    })
}

enum EmailPersistanceError {
    DatabaseError(diesel::result::Error),
    DuplicateEmail(String),
}

impl From<diesel::result::Error> for EmailPersistanceError {
    fn from(err: diesel::result::Error) -> Self {
        EmailPersistanceError::DatabaseError(err)
    }
}
