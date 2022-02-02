use anyhow::Result;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use password_hash::SaltString;
use rand_core::OsRng;

use super::Password;

/// The hash of a user's password.
#[derive(Clone, Debug)]
pub struct Hash(String);

impl Hash {
    /// Construct a new hash from a user's password.
    ///
    /// # Arguments
    ///
    /// * `password` - The user's password.
    ///
    /// # Returns
    ///
    /// Returns a [`Result`] containing the hashed password if the operation
    /// completed successfully.
    pub fn new(password: &Password) -> Result<Self> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(password.as_bytes(), salt.as_ref())?
            .to_string();

        Ok(Self(password_hash))
    }

    /// Construct a hash from its string representation.
    ///
    /// This is intended for creating the hash from a hash value that has been
    /// persisted.
    ///
    /// # Arguments
    ///
    /// * `hash` - The string representation of the hash;
    ///
    /// # Returns
    ///
    /// A [`Result`] containing the parsed hash. This will contain an [`Err`]
    /// variant if the provided string is not a valid hash.
    pub fn from_hash_str(hash: &str) -> Result<Self> {
        Ok(Self(PasswordHash::new(hash)?.to_string()))
    }

    /// Determine if the hash value matches a raw password.
    ///
    /// # Arguments
    ///
    /// * `raw_password` - The password to compare the hash to.
    ///
    /// # Returns
    ///
    /// A [`Result`] containing a [`bool`] that indicates if the password
    /// matches the hash.
    pub fn matches_raw_password(&self, raw_password: &str) -> Result<bool> {
        // This should be valid due to a `Hash` only being creatable from valid
        // data.
        let parsed_hash = PasswordHash::new(&self.0)?;

        match Argon2::default().verify_password(raw_password.as_bytes(), &parsed_hash) {
            Ok(()) => Ok(true),
            Err(password_hash::Error::Password) => Ok(false),
            Err(other) => Err(other.into()),
        }
    }

    /// Retrieve the hash's string representation.
    ///
    /// # Returns
    ///
    /// The hash's string representation. See [`PasswordHash`] for details on
    /// the format.
    pub fn value(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn new_hash_matches_password() {
        let raw_password = "hunter2";
        let hash = Hash::new(&Password::unvalidated(raw_password.to_owned()))
            .expect("Password should hash with no validators");

        let password_matches = hash
            .matches_raw_password(raw_password)
            .expect("Comparison should not error");

        assert!(password_matches, "Password does not match its own hash.");
    }

    #[test]
    fn new_hash_does_not_match_other_passwords() {
        let raw_password = "hunter2";
        let hash = Hash::new(&Password::unvalidated(raw_password.to_owned()))
            .expect("Password should hash with no validators");

        let password_matches = hash
            .matches_raw_password("not-the-password")
            .expect("Comparison should not error");

        assert!(
            !password_matches,
            "Password matched hash of different password."
        );
    }
}
