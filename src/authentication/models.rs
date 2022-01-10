use anyhow::Result;
use diesel::{result::DatabaseErrorKind, PgConnection, Queryable};
use uuid::Uuid;

#[derive(Debug, Queryable)]
pub struct User {
    pub id: Uuid,
    #[column_name = "password"]
    pub password_hash: String,
}

impl User {
    pub fn by_email(conn: &PgConnection, email: &str) -> Result<Option<Self>> {
        use crate::schema::{email, user};
        use diesel::prelude::*;

        user::table
            .select((user::id, user::password))
            .inner_join(email::table)
            .filter(
                email::provided_address
                    .eq(email)
                    .and(email::verified_at.is_not_null()),
            )
            .first::<Self>(conn)
            .optional()
            .map_err(|err| anyhow::Error::from(err))
    }
}
