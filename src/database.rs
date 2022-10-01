use std::ops::Deref;

use sqlx::PgPool;

#[derive(Clone)]
pub struct PostgresConnection(PgPool);

impl PostgresConnection {
    pub fn new(pool: PgPool) -> Self {
        Self(pool)
    }
}

impl Deref for PostgresConnection {
    type Target = PgPool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
