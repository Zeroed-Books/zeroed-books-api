#[macro_use]
extern crate rocket;

use rocket_sync_db_pools::{database, diesel};

#[database("postgres")]
struct PostgresConn(diesel::PgConnection);

#[get("/")]
fn index(_conn: PostgresConn) -> &'static str {
    "Hello, world!"
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(PostgresConn::fairing())
        .mount("/", routes![index])
}
