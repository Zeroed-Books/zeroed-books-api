#[macro_use]
extern crate diesel;
#[macro_use]
extern crate rocket;

use rocket::serde::{json::Json, Deserialize, Serialize};

pub mod models;
pub mod schema;

#[derive(Deserialize)]
pub struct NewUserRequest<'r> {
    email: &'r str,
    password: &'r str,
}

#[derive(Serialize)]
pub struct NewUserResponse<'r> {
    email: &'r str,
}

#[post("/users", data = "<new_user>")]
pub fn create_user(new_user: Json<NewUserRequest<'_>>) -> Json<NewUserResponse> {
    Json(NewUserResponse {
        email: new_user.email,
    })
}
