use tracing::debug;

use zeroed_books_api::cli;

#[rocket::main]
async fn main() -> anyhow::Result<()> {
    debug!("Starting CLI.");

    cli::run_with_sys_args().await
}
