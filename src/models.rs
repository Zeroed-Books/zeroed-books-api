use std::convert::TryFrom;

use crate::identities::domain::users::NewUser;
use uuid::Uuid;

#[derive(Clone)]
pub struct NewUserModel {
    pub id: Uuid,
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
