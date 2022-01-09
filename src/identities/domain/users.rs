use anyhow::Result;
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use rand_core::OsRng;
use uuid::Uuid;

pub struct NewUser {
    id: Uuid,
    password_hash: String,
}

impl NewUser {
    /// Create a representation of a new user.
    ///
    /// # Arguments
    ///
    /// * `password` - The user's raw password as a string.
    pub fn new(password: &str) -> Result<Self> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password_simple(password.as_bytes(), salt.as_ref())?
            .to_string();

        Ok(Self {
            id: Uuid::new_v4(),
            password_hash,
        })
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn password_hash(&self) -> &str {
        &self.password_hash
    }
}

#[cfg(test)]
mod tests {
    use argon2::{PasswordHash, PasswordVerifier};

    use super::*;

    #[test]
    fn new_user_password_hash_matches() {
        let sample_password = "hunter2";
        let new_user = NewUser::new(sample_password).expect("user creation should succeed");

        let parsed_hash =
            PasswordHash::new(new_user.password_hash()).expect("invalid password hash");

        assert!(Argon2::default()
            .verify_password(sample_password.as_bytes(), &parsed_hash)
            .is_ok());
    }
}
