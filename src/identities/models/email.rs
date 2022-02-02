use diesel::{
    insert_into, result::DatabaseErrorKind, result::Error as DieselError, Insertable, PgConnection,
};
use uuid::Uuid;

use crate::{
    identities::domain::email::{Email, EmailVerification},
    schema::{email, email_verification},
};

#[derive(Clone, Debug, Insertable)]
#[table_name = "email"]
pub struct NewEmail {
    id: Uuid,
    user_id: Uuid,
    provided_address: String,
    normalized_address: String,
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
            id: Uuid::new_v4(),
            user_id,
            provided_address: email.address().to_owned(),
            normalized_address: email.address().to_owned(),
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
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

#[derive(Debug, Insertable)]
#[table_name = "email_verification"]
pub struct NewEmailVerification {
    token: String,
    email_id: Uuid,
}

impl NewEmailVerification {
    pub fn new(email_id: Uuid, verification: &EmailVerification) -> Self {
        Self {
            token: verification.token().to_string(),
            email_id,
        }
    }

    pub fn save(&self, conn: &PgConnection) -> Result<(), DieselError> {
        use crate::schema::email_verification::dsl::*;
        use diesel::prelude::*;

        insert_into(email_verification)
            .values(self)
            .execute(conn)
            .map(|_| ())
    }
}
