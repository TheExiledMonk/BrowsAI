use browsai_profiles::ProfileId;
use std::collections::BTreeMap;
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PasskeyHandle(Uuid);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebAuthnError {
    InvalidRpOrigin,
    ProfileMismatch,
    OriginMismatch,
    UnknownCredential,
    UnknownChallenge,
}

#[derive(Clone, Debug)]
struct PasskeyRecord {
    profile: ProfileId,
    rp_origin: String,
    user_handle: Vec<u8>,
}

#[derive(Default)]
pub struct WebAuthnBroker {
    credentials: BTreeMap<PasskeyHandle, PasskeyRecord>,
    challenges: std::collections::HashSet<Vec<u8>>,
}

impl WebAuthnBroker {
    pub fn register(
        &mut self,
        profile: ProfileId,
        rp_origin: &str,
        user_handle: Vec<u8>,
    ) -> Result<PasskeyHandle, WebAuthnError> {
        validate_origin(rp_origin)?;
        let handle = PasskeyHandle(Uuid::new_v4());
        self.credentials.insert(
            handle.clone(),
            PasskeyRecord {
                profile,
                rp_origin: rp_origin.into(),
                user_handle,
            },
        );
        Ok(handle)
    }
    pub fn begin_assertion(
        &mut self,
        handle: &PasskeyHandle,
        profile: &ProfileId,
        origin: &str,
        user_verification: bool,
    ) -> Result<Vec<u8>, WebAuthnError> {
        validate_origin(origin)?;
        let record = self
            .credentials
            .get(handle)
            .ok_or(WebAuthnError::UnknownCredential)?;
        if &record.profile != profile {
            return Err(WebAuthnError::ProfileMismatch);
        }
        if record.rp_origin != origin {
            return Err(WebAuthnError::OriginMismatch);
        }
        let mut challenge = Uuid::new_v4().as_bytes().to_vec();
        if user_verification {
            challenge.push(1);
        } else {
            challenge.push(0);
        }
        self.challenges.insert(challenge.clone());
        Ok(challenge)
    }

    pub fn complete_assertion(&mut self, challenge: &[u8]) -> Result<(), WebAuthnError> {
        if self.challenges.remove(challenge) {
            Ok(())
        } else {
            Err(WebAuthnError::UnknownChallenge)
        }
    }
    pub fn user_handle(&self, handle: &PasskeyHandle) -> Option<&[u8]> {
        self.credentials
            .get(handle)
            .map(|record| record.user_handle.as_slice())
    }
}

fn validate_origin(origin: &str) -> Result<(), WebAuthnError> {
    let url = Url::parse(origin).map_err(|_| WebAuthnError::InvalidRpOrigin)?;
    if !matches!(url.scheme(), "https" | "http") || url.host_str().is_none() || url.path() != "/" {
        return Err(WebAuthnError::InvalidRpOrigin);
    }
    Ok(())
}
