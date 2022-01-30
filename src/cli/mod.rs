use clap::{Args, Parser, Subcommand};

use crate::server;

mod migrate;

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Migrate(MigrateOpts),
    Serve(ServeOpts),
}

#[derive(Args)]
struct MigrateOpts {
    /// Connection string for the database.
    #[clap(long = "database-url", env = "DATABASE_URL")]
    database_url: String,
}

impl From<&MigrateOpts> for migrate::MigrationOpts {
    fn from(opts: &MigrateOpts) -> Self {
        Self {
            database_url: opts.database_url.to_owned(),
        }
    }
}

#[derive(Args)]
struct ServeOpts {
    /// The number of connections to use for the database pool.
    #[clap(long = "database-pool-size", default_value = "16")]
    database_pool_size: u32,

    /// The number of seconds before a database connection times out.
    #[clap(long = "database-timeout", default_value = "5")]
    database_timeout: u8,

    /// Connection string for the application database.
    #[clap(long = "database-url", env = "DATABASE_URL")]
    database_url: String,

    /// Connection string for Redis.
    #[clap(long = "redis-url", env = "REDIS_URL")]
    redis_url: String,

    /// Secret key for signing application data.
    ///
    /// If this is changed, existing session cookies will become invalid.
    /// Generate with: openssl rand -base64 32
    #[clap(long = "secret-key", env = "SECRET_KEY")]
    secret_key: String,

    /// API key for SendGrid.
    ///
    /// If provided, emails will be sent using SendGrid. If this is not set,
    /// emails will be printed to stdout.
    #[clap(long = "sendgrid-key", env = "SENDGRID_KEY")]
    sendgrid_key: Option<String>,
}

impl From<&ServeOpts> for server::Options {
    fn from(opts: &ServeOpts) -> Self {
        Self {
            database_pool_size: opts.database_pool_size,
            database_timeout_seconds: opts.database_timeout,
            database_url: opts.database_url.to_owned(),
            redis_url: opts.redis_url.to_owned(),
            secret_key: opts.secret_key.to_owned(),
            sendgrid_key: opts.sendgrid_key.to_owned(),
        }
    }
}

pub async fn run_with_sys_args() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Migrate(opts) => Ok(migrate::run_migrations(opts.into())?),
        Commands::Serve(opts) => Ok(server::rocket(opts.into())?
            .ignite()
            .await
            .expect("Rocket ignition failure")
            .launch()
            .await
            .map(|_| ())?),
    }
}
