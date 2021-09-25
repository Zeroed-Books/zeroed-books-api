use diesel::{
    insert_into, result::DatabaseErrorKind, result::Error as DieselError, Insertable, PgConnection,
};
use uuid::Uuid;

use crate::{identities::domain::email::Email, schema::email};

#[derive(Clone, Debug, Insertable)]
#[table_name = "email"]
pub struct NewEmail {
    provided_address: String,
    normalized_address: String,
    user_id: Uuid,
}

impl NewEmail {
    /// Create an email that can be inserted into the database belonging to a
    /// specific user.
    ///
    /// # Arguments
    ///
    /// * `user_id` - The ID of the user who owns the email address.
    /// * `email` - The email address to persist.
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

    pub fn save(&self, conn: &PgConnection) -> Result<(), EmailPersistanceError> {
        use crate::schema::email::dsl::*;
        use diesel::prelude::*;

        match insert_into(email).values(self).execute(conn) {
            Ok(_) => Ok(()),
            Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
                Err(EmailPersistanceError::DuplicateEmail(self.clone()))
            }
            Err(err) => Err(err.into()),
        }
    }
}

#[derive(Debug)]
pub enum EmailPersistanceError {
    DatabaseError(diesel::result::Error),
    DuplicateEmail(NewEmail),
}

impl From<diesel::result::Error> for EmailPersistanceError {
    fn from(err: diesel::result::Error) -> Self {
        EmailPersistanceError::DatabaseError(err)
    }
}
