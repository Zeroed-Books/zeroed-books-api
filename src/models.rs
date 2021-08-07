use crate::schema::{email, user};
use diesel::Insertable;
use uuid::Uuid;

#[derive(Insertable)]
#[table_name = "email"]
pub struct NewEmail {
    pub provided_address: String,
    pub normalized_address: String,
    pub user_id: Uuid,
}

#[derive(Insertable)]
#[table_name = "user"]
pub struct NewUser {
    pub id: Uuid,
}
