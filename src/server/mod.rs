use rocket::{Build, Rocket};
use tera::Tera;

use crate::{
    create_user,
    email::clients::{ConsoleMailer, EmailClient, SendgridMailer},
    rate_limit::{RateLimiter, RedisRateLimiter},
    verify_email, PostgresConn,
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

    let redis_uri: String = figment
        .extract_inner("redis_url")
        .expect("No REDIS_URL provided");
    let rate_limiter: Box<dyn RateLimiter> =
        Box::new(RedisRateLimiter::new(&redis_uri).expect("failed to create Redis rate limiter"));

    rocket
        .attach(PostgresConn::fairing())
        .manage(email_client)
        .manage(rate_limiter)
        .manage(tera)
        .mount("/", routes![create_user, verify_email])
}
