use browsai_credentials::{CredentialBroker, CredentialId};
use browsai_profiles::ProfileId;
use browsai_secret_store::{SecretCapability, SecretError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoginForm {
    pub origin: String,
    pub username_field: String,
    pub password_field: String,
}

#[derive(Clone, Debug)]
pub struct AutofillPlan {
    pub credential_id: CredentialId,
    pub username_field: String,
    pub password_field: String,
    pub password_capability: SecretCapability,
}

#[derive(Default)]
pub struct PasswordManager {
    broker: CredentialBroker,
}

impl PasswordManager {
    pub fn detect_login_form(
        origin: impl Into<String>,
        fields: &[(&str, &str)],
    ) -> Option<LoginForm> {
        let username = fields.iter().find(|(name, kind)| {
            matches!(
                (
                    name.to_ascii_lowercase().as_str(),
                    kind.to_ascii_lowercase().as_str()
                ),
                ("username" | "user" | "email" | "login", _) | (_, "username" | "email")
            )
        })?;
        let password = fields.iter().find(|(name, kind)| {
            name.eq_ignore_ascii_case("password") || kind.eq_ignore_ascii_case("password")
        })?;
        Some(LoginForm {
            origin: origin.into(),
            username_field: username.0.to_owned(),
            password_field: password.0.to_owned(),
        })
    }

    pub fn save(
        &mut self,
        profile: ProfileId,
        origin: impl Into<String>,
        username: impl Into<String>,
        password: impl AsRef<[u8]>,
    ) -> CredentialId {
        self.broker
            .save_password(profile, origin, username, password)
    }
    pub fn plan_autofill(
        &self,
        profile: &ProfileId,
        form: &LoginForm,
    ) -> Option<(CredentialId, String)> {
        self.broker
            .find(profile, &form.origin)
            .into_iter()
            .next()
            .map(|record| (record.id.clone(), record.username.clone()))
    }

    pub fn update(
        &mut self,
        id: &CredentialId,
        password: impl AsRef<[u8]>,
    ) -> Result<(), SecretError> {
        self.broker.update_password(id, password)
    }
    pub fn authorize_password(
        &mut self,
        id: &CredentialId,
        profile: ProfileId,
        origin: &str,
    ) -> Result<SecretCapability, SecretError> {
        self.broker
            .grant_password(id, profile, origin, "password-autofill")
    }

    pub fn authorize_password_with_consent(
        &mut self,
        id: &CredentialId,
        profile: ProfileId,
        origin: &str,
        consent: bool,
    ) -> Result<SecretCapability, SecretError> {
        self.broker
            .grant_password_with_consent(id, profile, origin, "password-autofill", consent)
    }
    pub fn with_password<T>(
        &mut self,
        capability: &SecretCapability,
        profile: &ProfileId,
        origin: &str,
        use_password: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, SecretError> {
        self.broker
            .with_password(capability, profile, origin, use_password)
    }
}
