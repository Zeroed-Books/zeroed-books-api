use std::borrow::Cow;

use clap::{Args, Parser, Subcommand};
use tracing::debug;
use tracing_subscriber::EnvFilter;

use crate::server;

mod migrate;

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,

    /// DSN to tell Sentry where to send events.
    ///
    /// If provided, errors will be sent to Sentry.
    #[clap(long = "sentry-dsn", env = "SENTRY_DSN")]
    sentry_dsn: Option<String>,
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

impl From<MigrateOpts> for migrate::MigrationOpts {
    fn from(opts: MigrateOpts) -> Self {
        Self {
            database_url: opts.database_url,
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

    /// Address to send emails from.
    #[clap(
        long = "email-from-address",
        default_value = "admin@localhost",
        env = "EMAIL_FROM_ADDRESS"
    )]
    email_from_address: String,

    /// Display name to send emails from.
    #[clap(
        long = "email-from-name",
        default_value = "Zeroed Books",
        env = "EMAIL_FROM_NAME"
    )]
    email_from_name: String,

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

impl From<ServeOpts> for server::Options {
    fn from(opts: ServeOpts) -> Self {
        Self {
            database_pool_size: opts.database_pool_size,
            database_timeout_seconds: opts.database_timeout,
            database_url: opts.database_url,
            email_from_address: opts.email_from_address,
            email_from_name: opts.email_from_name,
            redis_url: opts.redis_url,
            secret_key: opts.secret_key,
            sendgrid_key: opts.sendgrid_key,
        }
    }
}

pub async fn run_with_sys_args() -> anyhow::Result<()> {
    use tracing_subscriber::prelude::*;

    let cli = Cli::parse();

    let sentry_config = cli.sentry_dsn.map(|dsn| {
        debug!("Enabled sentry.");

        let release_name = option_env!("GIT_SHA")
            .map(Cow::from)
            .or_else(|| sentry::release_name!());

        sentry::init((
            dsn,
            sentry::ClientOptions {
                release: release_name,
                ..Default::default()
            },
        ))
    });

    let sentry_tracing_layer = if sentry_config.is_some() {
        Some(sentry_tracing::layer())
    } else {
        None
    };

    let fmt_layer = tracing_subscriber::fmt::layer().with_filter(EnvFilter::from_default_env());

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(sentry_tracing_layer)
        .init();

    match cli.command {
        Commands::Migrate(opts) => Ok(migrate::run_migrations(opts.into()).await?),
        Commands::Serve(opts) => {
            let migrate_opts = MigrateOpts {
                database_url: opts.database_url.clone(),
            };

            migrate::run_migrations(migrate_opts.into()).await?;

            server::serve(opts.into()).await
        }
    }
}
