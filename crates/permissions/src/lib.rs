use browsai_profiles::ProfileId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Permission {
    Files,
    ClipboardRead,
    ClipboardWrite,
    Downloads,
    Uploads,
    Network,
    Geolocation,
    Notifications,
    Camera,
    Microphone,
    Certificates,
    Proxy,
    Device,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PermissionDecision {
    Granted,
    Denied,
    Prompt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub origin: String,
    pub permission: Permission,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PermissionGrant {
    pub origin: String,
    pub permission: Permission,
    pub expires_at_unix_seconds: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PermissionEvent {
    Requested {
        profile: ProfileId,
        origin: String,
        permission: Permission,
    },
    Decided {
        profile: ProfileId,
        origin: String,
        permission: Permission,
        decision: PermissionDecision,
    },
    Revoked {
        profile: ProfileId,
        origin: String,
        permission: Permission,
    },
    Expired {
        profile: ProfileId,
        origin: String,
        permission: Permission,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct PermissionKey {
    profile: ProfileId,
    origin: String,
    permission: Permission,
}

#[derive(Clone, Debug, Default)]
pub struct PermissionManager {
    grants: BTreeMap<PermissionKey, PermissionGrant>,
    decisions: BTreeMap<PermissionKey, PermissionDecision>,
    events: Vec<PermissionEvent>,
}

#[derive(Serialize, Deserialize)]
struct PermissionWire {
    profile: ProfileId,
    origin: String,
    permission: Permission,
    decision: PermissionDecision,
    grant: Option<PermissionGrant>,
}

impl PermissionManager {
    pub fn set(
        &mut self,
        profile: &ProfileId,
        origin: impl Into<String>,
        permission: Permission,
        granted: bool,
    ) {
        let key = PermissionKey {
            profile: profile.clone(),
            origin: origin.into(),
            permission,
        };
        let event_origin = key.origin.clone();
        if granted {
            self.grants.insert(
                key.clone(),
                PermissionGrant {
                    origin: key.origin.clone(),
                    permission,
                    expires_at_unix_seconds: None,
                },
            );
            self.decisions.insert(key, PermissionDecision::Granted);
            self.events.push(PermissionEvent::Decided {
                profile: profile.clone(),
                origin: event_origin,
                permission,
                decision: PermissionDecision::Granted,
            });
        } else {
            self.grants.remove(&key);
            self.decisions.insert(key, PermissionDecision::Denied);
            self.events.push(PermissionEvent::Decided {
                profile: profile.clone(),
                origin: event_origin,
                permission,
                decision: PermissionDecision::Denied,
            });
        }
    }

    pub fn grant_for(
        &mut self,
        profile: &ProfileId,
        request: &PermissionRequest,
        lifetime: Option<Duration>,
    ) -> PermissionGrant {
        let expires_at_unix_seconds =
            lifetime.map(|duration| now_seconds().saturating_add(duration.as_secs()));
        let grant = PermissionGrant {
            origin: request.origin.clone(),
            permission: request.permission,
            expires_at_unix_seconds,
        };
        self.grants.insert(
            PermissionKey {
                profile: profile.clone(),
                origin: request.origin.clone(),
                permission: request.permission,
            },
            grant.clone(),
        );
        self.decisions.insert(
            PermissionKey {
                profile: profile.clone(),
                origin: request.origin.clone(),
                permission: request.permission,
            },
            PermissionDecision::Granted,
        );
        self.events.push(PermissionEvent::Decided {
            profile: profile.clone(),
            origin: request.origin.clone(),
            permission: request.permission,
            decision: PermissionDecision::Granted,
        });
        grant
    }
    pub fn check(
        &self,
        profile: &ProfileId,
        origin: &str,
        permission: Permission,
    ) -> PermissionDecision {
        match self.grants.get(&PermissionKey {
            profile: profile.clone(),
            origin: origin.into(),
            permission,
        }) {
            Some(grant)
                if grant
                    .expires_at_unix_seconds
                    .map_or(true, |expires| expires > now_seconds()) =>
            {
                PermissionDecision::Granted
            }
            Some(_) => PermissionDecision::Prompt,
            None => self
                .decisions
                .get(&PermissionKey {
                    profile: profile.clone(),
                    origin: origin.into(),
                    permission,
                })
                .copied()
                .unwrap_or(PermissionDecision::Prompt),
        }
    }
    pub fn revoke(&mut self, profile: &ProfileId, origin: &str, permission: Permission) {
        let key = PermissionKey {
            profile: profile.clone(),
            origin: origin.into(),
            permission,
        };
        self.grants.remove(&key);
        self.decisions.remove(&key);
        self.events.push(PermissionEvent::Revoked {
            profile: profile.clone(),
            origin: origin.into(),
            permission,
        });
    }

    pub fn request(
        &mut self,
        profile: &ProfileId,
        request: &PermissionRequest,
    ) -> PermissionDecision {
        self.events.push(PermissionEvent::Requested {
            profile: profile.clone(),
            origin: request.origin.clone(),
            permission: request.permission,
        });
        self.check(profile, &request.origin, request.permission)
    }

    pub fn grants<'a>(
        &'a self,
        profile: &'a ProfileId,
    ) -> impl Iterator<Item = &'a PermissionGrant> + 'a {
        self.grants
            .iter()
            .filter(move |(key, _)| &key.profile == profile)
            .map(|(_, grant)| grant)
    }

    pub fn purge_expired(&mut self) -> usize {
        let now = now_seconds();
        let expired: Vec<_> = self
            .grants
            .iter()
            .filter(|(_, grant)| {
                grant
                    .expires_at_unix_seconds
                    .is_some_and(|expires| expires <= now)
            })
            .map(|(key, _)| key.clone())
            .collect();
        for key in &expired {
            self.grants.remove(key);
            if self.decisions.get(key) == Some(&PermissionDecision::Granted) {
                self.decisions.remove(key);
            }
            self.events.push(PermissionEvent::Expired {
                profile: key.profile.clone(),
                origin: key.origin.clone(),
                permission: key.permission,
            });
        }
        expired.len()
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let keys: std::collections::BTreeSet<_> = self
            .grants
            .keys()
            .chain(self.decisions.keys())
            .cloned()
            .collect();
        let entries = keys
            .into_iter()
            .map(|key| PermissionWire {
                profile: key.profile.clone(),
                origin: key.origin.clone(),
                permission: key.permission,
                decision: self
                    .decisions
                    .get(&key)
                    .copied()
                    .unwrap_or(PermissionDecision::Granted),
                grant: self.grants.get(&key).cloned(),
            })
            .collect::<Vec<_>>();
        serde_json::to_string(&entries)
    }

    pub fn from_json(value: &str) -> Result<Self, serde_json::Error> {
        let entries: Vec<PermissionWire> = serde_json::from_str(value)?;
        let mut manager = Self::default();
        for entry in entries {
            let key = PermissionKey {
                profile: entry.profile,
                origin: entry.origin,
                permission: entry.permission,
            };
            manager.decisions.insert(key.clone(), entry.decision);
            if let Some(grant) = entry.grant {
                manager.grants.insert(key, grant);
            }
        }
        Ok(manager)
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = PermissionEvent> + '_ {
        self.events.drain(..)
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
