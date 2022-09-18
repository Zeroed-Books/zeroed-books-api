use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug)]
pub struct User {
    pub id: Uuid,
    pub password_hash: String,
}

impl User {
    pub async fn by_email(pool: &PgPool, email: &str) -> Result<Option<Self>> {
        Ok(sqlx::query_as!(
            Self,
            r#"
            SELECT u.id, u.password AS password_hash
            FROM "email" e
            LEFT JOIN "user" u ON e.user_id = u.id
            WHERE e.provided_address = $1 AND e.verified_at IS NOT NULL
            "#,
            email
        )
        .fetch_optional(pool)
        .await?)
    }
}
