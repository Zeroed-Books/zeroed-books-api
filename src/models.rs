use std::convert::TryFrom;

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

impl TryFrom<&NewUser> for NewUserModel {
    type Error = anyhow::Error;

    fn try_from(user: &NewUser) -> Result<Self, Self::Error> {
        Ok(Self {
            id: user.id(),
            password_hash: user.password_hash()?.value().to_owned(),
        })
    }
}
