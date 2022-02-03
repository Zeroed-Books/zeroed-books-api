use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::schema::password_resets;

#[derive(Insertable)]
#[table_name = "password_resets"]
pub struct NewPasswordReset {
    pub token: String,
    pub user_id: Uuid,
}

#[derive(Queryable)]
pub struct PasswordReset {
    pub token: String,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
}
