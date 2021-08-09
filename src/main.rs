#[macro_use]
extern crate rocket;

use zeroed_books_api::{create_user, PostgresConn};

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(PostgresConn::fairing())
        .mount("/", routes![create_user])
}
