use diesel::Connection;

embed_migrations!();

pub struct MigrationOpts {
    pub database_url: String,
}

pub fn run_migrations(opts: MigrationOpts) -> anyhow::Result<()> {
    let connection = diesel::PgConnection::establish(&opts.database_url)?;

    Ok(embedded_migrations::run_with_output(
        &connection,
        &mut std::io::stdout(),
    )?)
}
