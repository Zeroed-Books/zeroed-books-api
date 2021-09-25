use rocket::{Build, Rocket};
use tera::Tera;

use crate::{
    create_user,
    email::clients::{ConsoleMailer, EmailClient, SendgridMailer},
    PostgresConn,
};

pub fn rocket() -> Rocket<Build> {
    let rocket = rocket::build();
    let figment = rocket.figment();

    let sendgrid_key: Option<String> = match figment.extract_inner("sendgrid_key") {
        Ok(key) => Some(key),
        Err(_) => None,
    };

    let email_client: Box<dyn EmailClient> = if let Some(api_key) = sendgrid_key {
        Box::new(SendgridMailer::new(api_key))
    } else {
        Box::new(ConsoleMailer {})
    };

    let tera = match Tera::new("templates/**/*") {
        Ok(t) => t,
        Err(e) => panic!("{}", e),
    };

    rocket
        .attach(PostgresConn::fairing())
        .manage(email_client)
        .manage(tera)
        .mount("/", routes![create_user])
}
