use crate::{
    email::Email,
    schema::{email, user},
};
use diesel::Insertable;
use uuid::Uuid;

#[derive(Insertable)]
#[table_name = "email"]
pub struct NewEmail {
    provided_address: String,
    normalized_address: String,
    user_id: Uuid,
}

impl NewEmail {
    pub fn for_user(user_id: Uuid, email: &Email) -> Self {
        Self {
            provided_address: email.provided_address().to_owned(),
            normalized_address: email.normalized_address().to_owned(),
            user_id,
        }
    }

    pub fn provided_address(&self) -> &str {
        &self.provided_address
    }
}

#[derive(Insertable)]
#[table_name = "user"]
pub struct NewUser {
    pub id: Uuid,
    #[column_name = "password"]
    pub password_hash: String,
}
