use tracing::{debug, error};

use zeroed_books_api::cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    debug!("Starting CLI.");

    if let Err(error) = cli::run_with_sys_args().await {
        error!(?error, "CLI failed to execute.");

        Err(error)
    } else {
        Ok(())
    }
}
