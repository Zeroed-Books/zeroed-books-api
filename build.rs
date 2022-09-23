use anyhow::Result;
use vergen::{vergen, Config};

fn main() -> Result<()> {
    // trigger recompilation when a new migration is added
    println!("cargo:rerun-if-changed=migrations-sqlx");

    vergen(Config::default())
}
