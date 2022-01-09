use crate::{identities::domain::users::NewUser, schema::user};
use diesel::Insertable;
use uuid::Uuid;

#[derive(Clone, Insertable)]
#[table_name = "user"]
pub struct NewUserModel {
    pub id: Uuid,
    #[column_name = "password"]
    pub password_hash: String,
}

impl From<NewUser> for NewUserModel {
    fn from(user: NewUser) -> Self {
        Self {
            id: user.id(),
            password_hash: user.password_hash().to_string(),
        }
    }
}
