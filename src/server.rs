use std::{sync::Arc, time::Duration};

use axum::{extract::FromRef, Router};
use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::{
    database::PostgresConnection, ledger::services::LedgerService,
    repos::transactions::DynTransactionRepo,
};

pub struct Options {
    pub database_pool_size: u32,
    pub database_timeout_seconds: u8,
    pub database_url: String,

    pub jwt_audience: String,
    pub jwt_authority: String,
}

#[derive(Clone)]
pub struct AppState {
    db: PgPool,
    jwks: axum_jwks::Jwks,
    ledger_service: LedgerService,
}

pub async fn serve(opts: Options) -> anyhow::Result<()> {
    let db_pool = PgPoolOptions::new()
        .max_connections(opts.database_pool_size)
        .acquire_timeout(Duration::from_secs(opts.database_timeout_seconds.into()))
        .connect(&opts.database_url)
        .await?;

    let jwks = axum_jwks::Jwks::from_authority(&opts.jwt_authority, opts.jwt_audience).await?;

    let db_connection = PostgresConnection::new(db_pool.clone());

    let transaction_repo: DynTransactionRepo = Arc::new(db_connection.clone());

    let ledger_service = LedgerService::new(transaction_repo);

    let state = AppState {
        db: db_pool,
        jwks,
        ledger_service,
    };

    let app = Router::new()
        .nest("/ledger", crate::ledger::http::routes())
        .with_state(state);

    axum::Server::bind(&"0.0.0.0:8000".parse().unwrap())
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

impl FromRef<AppState> for axum_jwks::Jwks {
    fn from_ref(state: &AppState) -> Self {
        state.jwks.clone()
    }
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}

impl FromRef<AppState> for LedgerService {
    fn from_ref(state: &AppState) -> Self {
        state.ledger_service.clone()
    }
}
