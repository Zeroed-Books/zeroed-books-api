use sqlx::{migrate::Migrator, postgres::PgPoolOptions};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub struct MigrationOpts {
    pub database_url: String,
}

pub async fn run_migrations(opts: MigrationOpts) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new().connect(&opts.database_url).await?;

    MIGRATOR.run(&pool).await?;

    Ok(())
}
