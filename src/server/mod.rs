use rocket::{Build, Rocket};

use crate::{create_user, PostgresConn};

pub fn rocket() -> Rocket<Build> {
    rocket::build()
        .attach(PostgresConn::fairing())
        .mount("/", routes![create_user])
}
