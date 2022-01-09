use tracing::debug;

use zeroed_books_api::cli;

#[rocket::main]
async fn main() -> Result<(), ()> {
    // Configure a default tracing subscriber using the RUST_LOG environment
    // variable.
    tracing_subscriber::fmt::init();

    debug!("Starting CLI.");

    cli::run_with_sys_args().await
}
