use diesel::Connection;
use std::env;

embed_migrations!();

pub fn run_migrations() -> Result<(), diesel_migrations::RunMigrationsError> {
    let db_url = env::var("DATABASE_URL").expect("$DATABASE_URL is not set.");
    let connection =
        diesel::PgConnection::establish(&db_url).expect("Failed to establish database connection.");

    embedded_migrations::run_with_output(&connection, &mut std::io::stdout())
}
