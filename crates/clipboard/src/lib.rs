use browsai_permissions::{Permission, PermissionDecision, PermissionManager};
use browsai_profiles::ProfileId;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardError {
    Permission(PermissionDecision),
    MissingUserGesture,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct UserGestureToken(Uuid);

#[derive(Default)]
pub struct ClipboardBroker {
    values: BTreeMap<(ProfileId, String), String>,
    gestures: BTreeSet<UserGestureToken>,
}

impl ClipboardBroker {
    pub fn write(
        &mut self,
        permissions: &PermissionManager,
        profile: &ProfileId,
        origin: &str,
        value: impl Into<String>,
    ) -> Result<(), ClipboardError> {
        let decision = permissions.check(profile, origin, Permission::ClipboardWrite);
        if !matches!(decision, PermissionDecision::Granted) {
            return Err(ClipboardError::Permission(decision));
        }
        self.values
            .insert((profile.clone(), origin.to_owned()), value.into());
        Ok(())
    }
    pub fn read(
        &self,
        permissions: &PermissionManager,
        profile: &ProfileId,
        origin: &str,
    ) -> Result<String, ClipboardError> {
        let decision = permissions.check(profile, origin, Permission::ClipboardRead);
        if !matches!(decision, PermissionDecision::Granted) {
            return Err(ClipboardError::Permission(decision));
        }
        Ok(self
            .values
            .get(&(profile.clone(), origin.to_owned()))
            .cloned()
            .unwrap_or_default())
    }

    pub fn issue_user_gesture(&mut self) -> UserGestureToken {
        let token = UserGestureToken(Uuid::new_v4());
        self.gestures.insert(token.clone());
        token
    }

    pub fn read_with_gesture(
        &mut self,
        permissions: &PermissionManager,
        profile: &ProfileId,
        origin: &str,
        token: UserGestureToken,
    ) -> Result<String, ClipboardError> {
        if !self.gestures.remove(&token) {
            return Err(ClipboardError::MissingUserGesture);
        }
        self.read(permissions, profile, origin)
    }
}
