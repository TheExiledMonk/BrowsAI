use browsai_profiles::ProfileId;
use browsai_secret_store::{SecretBroker, SecretCapability, SecretError};
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::collections::HashMap;
use uuid::Uuid;

type HmacSha1 = Hmac<Sha1>;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TotpId(Uuid);

#[derive(Clone, Debug)]
pub struct TotpRecord {
    pub id: TotpId,
    pub profile: ProfileId,
    pub origin: String,
    handle: browsai_secret_store::SecretHandle,
}

#[derive(Default)]
pub struct TotpBroker {
    secrets: SecretBroker,
    records: HashMap<TotpId, TotpRecord>,
}

impl TotpBroker {
    pub fn save_seed(
        &mut self,
        profile: ProfileId,
        origin: impl Into<String>,
        seed_base32: impl AsRef<[u8]>,
    ) -> TotpId {
        let origin = origin.into();
        let handle = self
            .secrets
            .store(profile.clone(), origin.clone(), seed_base32);
        let id = TotpId(Uuid::new_v4());
        self.records.insert(
            id.clone(),
            TotpRecord {
                id: id.clone(),
                profile,
                origin,
                handle,
            },
        );
        id
    }
    pub fn capability(
        &mut self,
        id: &TotpId,
        profile: ProfileId,
        origin: &str,
    ) -> Result<SecretCapability, SecretError> {
        let record = self.records.get(id).ok_or(SecretError::UnknownHandle)?;
        if record.profile != profile {
            return Err(SecretError::ProfileMismatch);
        }
        if record.origin != origin {
            return Err(SecretError::OriginMismatch);
        }
        self.secrets
            .grant(&record.handle, profile, origin, "totp", true)
    }
    pub fn with_code<T>(
        &mut self,
        capability: &SecretCapability,
        profile: &ProfileId,
        origin: &str,
        unix_seconds: u64,
        use_code: impl FnOnce(&str) -> T,
    ) -> Result<T, SecretError> {
        self.secrets
            .with_secret(capability, profile, origin, |seed| {
                let normalized = std::str::from_utf8(seed)
                    .map_err(|_| SecretError::InvalidSecret)?
                    .to_ascii_uppercase();
                let decoded = BASE32_NOPAD
                    .decode(normalized.as_bytes())
                    .map_err(|_| SecretError::InvalidSecret)?;
                if decoded.is_empty() {
                    return Err(SecretError::InvalidSecret);
                }
                let counter = unix_seconds / 30;
                let mut message = [0u8; 8];
                message.copy_from_slice(&counter.to_be_bytes());
                let mut mac =
                    HmacSha1::new_from_slice(&decoded).expect("HMAC accepts arbitrary key");
                mac.update(&message);
                let digest = mac.finalize().into_bytes();
                let offset = (digest[19] & 0x0f) as usize;
                let binary = ((u32::from(digest[offset]) & 0x7f) << 24)
                    | (u32::from(digest[offset + 1]) << 16)
                    | (u32::from(digest[offset + 2]) << 8)
                    | u32::from(digest[offset + 3]);
                let code = format!("{:06}", binary % 1_000_000);
                Ok(use_code(&code))
            })
            .and_then(|result| result)
    }
}
