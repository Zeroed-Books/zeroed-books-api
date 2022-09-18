use std::time::Duration;

use rocket::{Build, Rocket};
use sqlx::postgres::PgPoolOptions;
use tera::Tera;

use crate::{
    cors::CorsHeaders,
    create_user,
    email::clients::{ConsoleMailer, EmailClient, SendgridMailer},
    rate_limit::{RateLimiter, RedisRateLimiter},
    verify_email,
};

pub struct Options {
    pub database_pool_size: u32,
    pub database_timeout_seconds: u8,
    pub database_url: String,

    pub email_from_address: String,
    pub email_from_name: String,

    pub redis_url: String,

    pub secret_key: String,

    pub sendgrid_key: Option<String>,
}

pub async fn rocket(opts: Options) -> anyhow::Result<Rocket<Build>> {
    let figment = rocket::Config::figment().merge(("secret_key", &opts.secret_key));

    let db_pool = PgPoolOptions::new()
        .max_connections(opts.database_pool_size)
        .acquire_timeout(Duration::from_secs(opts.database_timeout_seconds.into()))
        .connect(&opts.database_url)
        .await?;

    let email_client: Box<dyn EmailClient> = if let Some(api_key) = opts.sendgrid_key {
        Box::new(SendgridMailer::new(
            api_key,
            opts.email_from_address,
            opts.email_from_name,
        ))
    } else {
        Box::new(ConsoleMailer {
            from: format!("{} <{}>", opts.email_from_name, opts.email_from_address),
        })
    };

    let tera = Tera::new("templates/**/*")?;

    let rate_limiter: Box<dyn RateLimiter> = Box::new(RedisRateLimiter::new(&opts.redis_url)?);

    Ok(rocket::custom(figment)
        .attach(CorsHeaders)
        .manage(db_pool)
        .manage(email_client)
        .manage(rate_limiter)
        .manage(tera)
        .mount("/", routes![create_user, verify_email])
        .mount("/authentication", crate::authentication::http::routes())
        .mount("/identities", crate::identities::http::routes())
        .mount("/ledger", crate::ledger::http::routes()))
}
