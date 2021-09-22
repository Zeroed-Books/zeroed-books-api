use rocket::{Build, Rocket};

use crate::{
    create_user,
    email::clients::{ConsoleMailer, EmailClient},
    PostgresConn,
};

pub fn rocket() -> Rocket<Build> {
    let emailClient: Box<&dyn EmailClient> = Box::new(&ConsoleMailer {});

    rocket::build()
        .attach(PostgresConn::fairing())
        .manage(emailClient)
        .mount("/", routes![create_user])
}
