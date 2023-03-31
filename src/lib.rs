// We often use a `new` constructor with required arguments to ensure that only
// structs with valid data can be created. A default implementation would avoid
// this benefit we get from the type system.
#![allow(clippy::new_without_default)]
#![deny(elided_lifetimes_in_paths)]

pub mod authentication;
pub mod cli;
mod database;
mod http_err;
pub mod ledger;
mod models;
mod repos;
mod server;
