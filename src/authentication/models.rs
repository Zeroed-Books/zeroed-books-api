use anyhow::Result;
use diesel::{dsl::Filter, PgConnection, Queryable};
use uuid::Uuid;

#[derive(Debug, Queryable)]
pub struct User {
    pub id: Uuid,
    #[column_name = "password"]
    pub password_hash: String,
}

impl User {
    pub fn by_email(conn: &PgConnection, email: &str) -> Result<Self> {
        use crate::schema::{email, user};
        use diesel::prelude::*;

        user::table
            .select((user::id, user::password))
            .inner_join(email::table)
            .filter(email::verified_at.is_not_null())
            .first::<Self>(conn)
            .map_err(|err| anyhow::Error::from(err))
    }
}
