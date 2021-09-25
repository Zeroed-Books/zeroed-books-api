use crate::schema::user;
use diesel::Insertable;
use uuid::Uuid;

#[derive(Clone, Insertable)]
#[table_name = "user"]
pub struct NewUser {
    pub id: Uuid,
    #[column_name = "password"]
    pub password_hash: String,
}
