use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    extract::FromRef,
    http::{header, Method},
    Router,
};
use axum_extra::extract::cookie::Key;
use sqlx::{postgres::PgPoolOptions, PgPool};
use tera::Tera;
use tower_http::cors::{self, CorsLayer};

use crate::{
    email::clients::{ConsoleMailer, EmailClient, SendgridMailer},
    rate_limit::{RateLimiter, RedisRateLimiter},
};

pub struct Options {
    pub database_pool_size: u32,
    pub database_timeout_seconds: u8,
    pub database_url: String,

    pub email_from_address: String,
    pub email_from_name: String,

    pub redis_url: String,

    pub secret_key: String,

    pub sendgrid_key: Option<String>,
}

#[derive(Clone)]
pub struct AppState {
    db: PgPool,
    email_client: Arc<dyn EmailClient>,
    key: Key,
    rate_limiter: Arc<dyn RateLimiter>,
    tera: Tera,
}

pub async fn serve(opts: Options) -> anyhow::Result<()> {
    let db_pool = PgPoolOptions::new()
        .max_connections(opts.database_pool_size)
        .acquire_timeout(Duration::from_secs(opts.database_timeout_seconds.into()))
        .connect(&opts.database_url)
        .await?;

    let email_client: Arc<dyn EmailClient> = if let Some(api_key) = opts.sendgrid_key {
        Arc::new(SendgridMailer::new(
            api_key,
            opts.email_from_address,
            opts.email_from_name,
        ))
    } else {
        Arc::new(ConsoleMailer {
            from: format!("{} <{}>", opts.email_from_name, opts.email_from_address),
        })
    };

    let tera = Tera::new("templates/**/*")?;

    let rate_limiter: Arc<dyn RateLimiter> = Arc::new(RedisRateLimiter::new(&opts.redis_url)?);

    let cors = CorsLayer::new()
        .allow_credentials(true)
        .allow_headers([header::CONTENT_TYPE])
        .allow_methods([Method::DELETE, Method::GET, Method::POST, Method::PUT])
        .allow_origin(cors::AllowOrigin::mirror_request());

    let state = AppState {
        db: db_pool,
        email_client,
        key: Key::from(opts.secret_key.as_bytes()),
        rate_limiter,
        tera,
    };

    let app = Router::new()
        .nest(
            "/authentication",
            crate::authentication::http::routes(state.clone()),
        )
        .nest(
            "/identities",
            crate::identities::http::routes(state.clone()),
        )
        .nest("/ledger", crate::ledger::http::routes(state.clone()))
        .layer(cors);

    axum::Server::bind(&"0.0.0.0:8000".parse().unwrap())
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;

    Ok(())
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}

impl FromRef<AppState> for Tera {
    fn from_ref(state: &AppState) -> Self {
        state.tera.clone()
    }
}

impl FromRef<AppState> for Arc<dyn EmailClient> {
    fn from_ref(state: &AppState) -> Self {
        state.email_client.clone()
    }
}

impl FromRef<AppState> for Arc<dyn RateLimiter> {
    fn from_ref(state: &AppState) -> Self {
        state.rate_limiter.clone()
    }
}
