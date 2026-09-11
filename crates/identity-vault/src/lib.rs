use browsai_profiles::ProfileId;
use browsai_secret_store::{SecretBroker, SecretCapability, SecretError};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IdentityId(Uuid);

#[derive(Clone, Debug)]
pub struct IdentityProfile {
    pub id: IdentityId,
    pub profile: ProfileId,
    pub origin: String,
    fields: BTreeMap<String, browsai_secret_store::SecretHandle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityAuditEvent {
    pub identity_id: IdentityId,
    pub field: Option<String>,
    pub action: &'static str,
}

#[derive(Default)]
pub struct IdentityVault {
    broker: SecretBroker,
    profiles: BTreeMap<IdentityId, IdentityProfile>,
    events: Vec<IdentityAuditEvent>,
}

impl IdentityVault {
    pub fn create(
        &mut self,
        profile: ProfileId,
        origin: impl Into<String>,
        fields: BTreeMap<String, Vec<u8>>,
    ) -> IdentityId {
        let origin = origin.into();
        let id = IdentityId(Uuid::new_v4());
        let handles = fields
            .into_iter()
            .map(|(name, value)| {
                (
                    name,
                    self.broker.store(profile.clone(), origin.clone(), value),
                )
            })
            .collect();
        self.profiles.insert(
            id.clone(),
            IdentityProfile {
                id: id.clone(),
                profile,
                origin,
                fields: handles,
            },
        );
        self.events.push(IdentityAuditEvent {
            identity_id: id.clone(),
            field: None,
            action: "created",
        });
        id
    }
    pub fn authorize_field(
        &mut self,
        id: &IdentityId,
        field: &str,
        profile: ProfileId,
        origin: &str,
    ) -> Result<SecretCapability, SecretError> {
        self.authorize_field_with_consent(id, field, profile, origin, true)
    }

    fn authorize_field_unchecked(
        &mut self,
        id: &IdentityId,
        field: &str,
        profile: ProfileId,
        origin: &str,
    ) -> Result<SecretCapability, SecretError> {
        let record = self.profiles.get(id).ok_or(SecretError::UnknownHandle)?;
        if record.profile != profile {
            return Err(SecretError::ProfileMismatch);
        }
        if record.origin != origin {
            return Err(SecretError::OriginMismatch);
        }
        let handle = record.fields.get(field).ok_or(SecretError::UnknownHandle)?;
        self.broker
            .grant(handle, profile, origin, format!("identity:{field}"), true)
    }

    pub fn authorize_field_with_consent(
        &mut self,
        id: &IdentityId,
        field: &str,
        profile: ProfileId,
        origin: &str,
        consent: bool,
    ) -> Result<SecretCapability, SecretError> {
        if !consent {
            return Err(SecretError::InvalidCapability);
        }
        let capability = self.authorize_field_unchecked(id, field, profile, origin)?;
        self.events.push(IdentityAuditEvent {
            identity_id: id.clone(),
            field: Some(field.to_owned()),
            action: "field_authorized",
        });
        Ok(capability)
    }
    pub fn with_field<T>(
        &mut self,
        capability: &SecretCapability,
        profile: &ProfileId,
        origin: &str,
        use_field: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, SecretError> {
        let result = self
            .broker
            .with_secret(capability, profile, origin, use_field);
        if result.is_ok() {
            self.events.push(IdentityAuditEvent {
                identity_id: self
                    .profiles
                    .iter()
                    .find(|(_, identity)| {
                        identity
                            .fields
                            .values()
                            .any(|handle| handle == &capability.handle)
                    })
                    .map(|(id, _)| id.clone())
                    .expect("capability was issued by this vault"),
                field: None,
                action: "field_released",
            });
        }
        result
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = IdentityAuditEvent> + '_ {
        self.events.drain(..)
    }
}
