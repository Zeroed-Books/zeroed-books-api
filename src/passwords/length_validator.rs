use super::{Remediation, ValidationError, Validator};

pub struct LengthValidator {
    min: usize,
    max: usize,
}

impl LengthValidator {
    pub fn new(min: usize, max: usize) -> Self {
        assert!(
            min <= max,
            "Min length must be less than or equal to max length."
        );

        Self { min, max }
    }
}

impl Default for LengthValidator {
    fn default() -> Self {
        Self { min: 8, max: 512 }
    }
}

#[async_trait]
impl Validator for LengthValidator {
    async fn validate(&self, password: &str) -> Result<(), ValidationError> {
        if password.len() < self.min {
            Err(ValidationError::FailsRule(Remediation {
                message: format!("Passwords must contain at least {} characters.", self.min),
            }))
        } else if password.len() > self.max {
            Err(ValidationError::FailsRule(Remediation {
                message: format!("Passwords may not be longer than {} characters.", self.max),
            }))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use rocket::tokio;

    use super::*;

    #[tokio::test]
    async fn too_short() {
        let validator = LengthValidator::new(2, 3);
        let password = "a";

        match validator
            .validate(password)
            .await
            .expect_err("Password is too short.")
        {
            ValidationError::FailsRule(_) => (),
            #[allow(unreachable_patterns)]
            other => assert!(false, "Unexpected error type: {:?}", other),
        }
    }

    #[tokio::test]
    async fn too_long() {
        let validator = LengthValidator::new(2, 4);
        let password = "a".repeat(5);

        match validator
            .validate(&password)
            .await
            .expect_err("Password is too short.")
        {
            ValidationError::FailsRule(_) => (),
            #[allow(unreachable_patterns)]
            other => assert!(false, "Unexpected error type: {:?}", other),
        }
    }

    #[tokio::test]
    async fn valid() {
        let validator = LengthValidator::new(5, 5);
        let password = "a".repeat(5);

        validator
            .validate(&password)
            .await
            .expect("Password meets criteria.");
    }
}
