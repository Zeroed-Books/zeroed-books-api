use zeroed_books_api::cli;

#[rocket::main]
async fn main() -> Result<(), ()> {
    cli::run_with_sys_args().await
}
