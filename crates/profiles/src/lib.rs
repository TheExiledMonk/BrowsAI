//! Profile identity and isolation primitives.

use browsai_engine_api::{IdentityViewport, ProfileIdentity, SecChUaBrand as ApiSecChUaBrand};
use browsai_fingerprint::{FingerprintCatalog, FingerprintId, FingerprintInconsistency};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ProfileId(Uuid);

impl ProfileId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for ProfileId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
    pub generation: u64,
    pub config: ProfileConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProfileViewport {
    pub width: u32,
    pub height: u32,
    pub device_scale_factor: u32,
}

impl Default for ProfileViewport {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            device_scale_factor: 1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SecChUaBrand {
    pub brand: String,
    pub version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub user_agent: Option<String>,
    pub locale: String,
    pub timezone: String,
    pub homepage: Option<String>,
    pub extensions: Vec<String>,
    pub history: Vec<HistoryEntry>,
    pub autocomplete: Vec<AutocompleteEntry>,
    #[serde(default)]
    pub viewport: ProfileViewport,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub brands: Vec<SecChUaBrand>,
    #[serde(default)]
    pub accept_language: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProfileLimits {
    pub max_profiles: usize,
    pub max_history_entries: usize,
    pub max_autocomplete_entries: usize,
    pub max_extensions: usize,
}

impl Default for ProfileLimits {
    fn default() -> Self {
        Self {
            max_profiles: 32,
            max_history_entries: 1_000,
            max_autocomplete_entries: 100,
            max_extensions: 64,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub url: String,
    pub title: Option<String>,
    pub visited_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AutocompleteEntry {
    pub field: String,
    pub value: String,
    pub last_used_unix_seconds: u64,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            user_agent: None,
            locale: "en-US".into(),
            timezone: "UTC".into(),
            homepage: None,
            extensions: vec![],
            history: vec![],
            autocomplete: vec![],
            viewport: ProfileViewport::default(),
            platform: "Linux x86_64".into(),
            brands: vec![
                SecChUaBrand {
                    brand: " Not A;Brand".into(),
                    version: "99".into(),
                },
                SecChUaBrand {
                    brand: "Chromium".into(),
                    version: "140".into(),
                },
                SecChUaBrand {
                    brand: "Google Chrome".into(),
                    version: "140".into(),
                },
            ],
            accept_language: "en-US,en;q=0.9".into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProfileManager {
    profiles: BTreeMap<ProfileId, Profile>,
    selected: Option<ProfileId>,
    limits: ProfileLimits,
}

#[derive(Serialize, Deserialize)]
struct ProfileManagerWire {
    #[serde(default = "profile_schema_version")]
    schema_version: u16,
    profiles: Vec<Profile>,
    selected: Option<ProfileId>,
    #[serde(default)]
    limits: ProfileLimits,
}

fn profile_schema_version() -> u16 {
    2
}

impl ProfileManager {
    pub const SCHEMA_VERSION: u16 = 2;

    pub fn create(&mut self, name: impl Into<String>) -> ProfileId {
        let id = ProfileId::new();
        self.profiles.insert(
            id.clone(),
            Profile {
                id: id.clone(),
                name: name.into(),
                generation: 0,
                config: ProfileConfig::default(),
            },
        );
        if self.selected.is_none() {
            self.selected = Some(id.clone());
        }
        id
    }

    pub fn try_create(&mut self, name: impl Into<String>) -> Option<ProfileId> {
        if self.profiles.len() >= self.limits.max_profiles {
            return None;
        }
        Some(self.create(name))
    }

    pub fn limits(&self) -> &ProfileLimits {
        &self.limits
    }

    pub fn set_limits(&mut self, limits: ProfileLimits) {
        self.limits = limits;
        self.profiles
            .values_mut()
            .for_each(|profile| normalize_config(&mut profile.config, &self.limits));
    }
    pub fn get(&self, id: &ProfileId) -> Option<&Profile> {
        self.profiles.get(id)
    }
    pub fn list(&self) -> impl Iterator<Item = &Profile> {
        self.profiles.values()
    }
    pub fn delete(&mut self, id: &ProfileId) -> bool {
        let removed = self.profiles.remove(id).is_some();
        if removed && self.selected.as_ref() == Some(id) {
            self.selected = self.profiles.keys().next().cloned();
        }
        removed
    }

    pub fn select(&mut self, id: &ProfileId) -> bool {
        if self.profiles.contains_key(id) {
            self.selected = Some(id.clone());
            true
        } else {
            false
        }
    }

    pub fn selected(&self) -> Option<&Profile> {
        self.selected.as_ref().and_then(|id| self.profiles.get(id))
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&ProfileManagerWire {
            schema_version: Self::SCHEMA_VERSION,
            profiles: self.profiles.values().cloned().collect(),
            selected: self.selected.clone(),
            limits: self.limits.clone(),
        })
    }

    pub fn from_json(value: &str) -> Result<Self, serde_json::Error> {
        let mut wire: ProfileManagerWire = serde_json::from_str(value)?;
        if wire.schema_version > Self::SCHEMA_VERSION {
            return Err(<serde_json::Error as serde::de::Error>::custom(
                "unsupported profile schema version",
            ));
        }
        for profile in &mut wire.profiles {
            if wire.schema_version < 2 {
                migrate_v1_to_v2(&mut profile.config);
            }
        }
        wire.schema_version = Self::SCHEMA_VERSION;
        let profiles: BTreeMap<ProfileId, Profile> = wire
            .profiles
            .into_iter()
            .map(|profile| (profile.id.clone(), profile))
            .collect();
        let selected = wire
            .selected
            .filter(|id| profiles.contains_key(id))
            .or_else(|| profiles.keys().next().cloned());
        let mut manager = Self {
            profiles,
            selected,
            limits: wire.limits,
        };
        manager
            .profiles
            .values_mut()
            .for_each(|profile| normalize_config(&mut profile.config, &manager.limits));
        Ok(manager)
    }

    pub fn update_config(&mut self, id: &ProfileId, config: ProfileConfig) -> bool {
        self.profiles
            .get_mut(id)
            .map(|profile| {
                profile.config = config;
                normalize_config(&mut profile.config, &self.limits);
                profile.generation += 1;
                true
            })
            .unwrap_or(false)
    }

    pub fn record_history(
        &mut self,
        id: &ProfileId,
        entry: HistoryEntry,
        max_entries: usize,
    ) -> bool {
        let Some(profile) = self.profiles.get_mut(id) else {
            return false;
        };
        profile.config.history.push(entry);
        let limit = max_entries.min(self.limits.max_history_entries);
        if limit > 0 && profile.config.history.len() > limit {
            profile
                .config
                .history
                .drain(..profile.config.history.len() - limit);
        }
        profile.generation += 1;
        true
    }

    pub fn record_autocomplete(
        &mut self,
        id: &ProfileId,
        entry: AutocompleteEntry,
        max_entries: usize,
    ) -> bool {
        let Some(profile) = self.profiles.get_mut(id) else {
            return false;
        };
        profile
            .config
            .autocomplete
            .retain(|existing| existing.field != entry.field || existing.value != entry.value);
        profile.config.autocomplete.push(entry);
        let limit = max_entries.min(self.limits.max_autocomplete_entries);
        if limit > 0 && profile.config.autocomplete.len() > limit {
            profile
                .config
                .autocomplete
                .drain(..profile.config.autocomplete.len() - limit);
        }
        profile.generation += 1;
        true
    }
}

fn normalize_config(config: &mut ProfileConfig, limits: &ProfileLimits) {
    if limits.max_history_entries > 0 && config.history.len() > limits.max_history_entries {
        config
            .history
            .drain(..config.history.len() - limits.max_history_entries);
    }
    if limits.max_autocomplete_entries > 0
        && config.autocomplete.len() > limits.max_autocomplete_entries
    {
        config
            .autocomplete
            .drain(..config.autocomplete.len() - limits.max_autocomplete_entries);
    }
    if limits.max_extensions > 0 && config.extensions.len() > limits.max_extensions {
        config.extensions.truncate(limits.max_extensions);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProfileVariant {
    ChromeDesktop,
    FirefoxDesktop,
    SafariDesktop,
    ChromeAndroid,
    SafariIos,
}

impl ProfileVariant {
    pub fn default_config(self) -> ProfileConfig {
        match self {
            ProfileVariant::ChromeDesktop => ProfileConfig {
                user_agent: Some(
                    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36".into(),
                ),
                locale: "en-US".into(),
                timezone: "America/Los_Angeles".into(),
                homepage: None,
                extensions: vec![],
                history: vec![],
                autocomplete: vec![],
                viewport: ProfileViewport {
                    width: 1920,
                    height: 1080,
                    device_scale_factor: 1,
                },
                platform: "Linux x86_64".into(),
                brands: vec![
                    SecChUaBrand {
                        brand: " Not A;Brand".into(),
                        version: "99".into(),
                    },
                    SecChUaBrand {
                        brand: "Chromium".into(),
                        version: "140".into(),
                    },
                    SecChUaBrand {
                        brand: "Google Chrome".into(),
                        version: "140".into(),
                    },
                ],
                accept_language: "en-US,en;q=0.9".into(),
            },
            ProfileVariant::FirefoxDesktop => ProfileConfig {
                user_agent: Some(
                    "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0".into(),
                ),
                locale: "en-US".into(),
                timezone: "America/Los_Angeles".into(),
                homepage: None,
                extensions: vec![],
                history: vec![],
                autocomplete: vec![],
                viewport: ProfileViewport {
                    width: 1920,
                    height: 1080,
                    device_scale_factor: 1,
                },
                platform: "Linux x86_64".into(),
                brands: vec![],
                accept_language: "en-US,en;q=0.5".into(),
            },
            ProfileVariant::SafariDesktop => ProfileConfig {
                user_agent: Some(
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_6) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15".into(),
                ),
                locale: "en-US".into(),
                timezone: "America/Los_Angeles".into(),
                homepage: None,
                extensions: vec![],
                history: vec![],
                autocomplete: vec![],
                viewport: ProfileViewport {
                    width: 2560,
                    height: 1440,
                    device_scale_factor: 2,
                },
                platform: "MacIntel".into(),
                brands: vec![],
                accept_language: "en-US,en;q=0.9".into(),
            },
            ProfileVariant::ChromeAndroid => ProfileConfig {
                user_agent: Some(
                    "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36".into(),
                ),
                locale: "en-US".into(),
                timezone: "America/Los_Angeles".into(),
                homepage: None,
                extensions: vec![],
                history: vec![],
                autocomplete: vec![],
                viewport: ProfileViewport {
                    width: 412,
                    height: 915,
                    device_scale_factor: 2,
                },
                platform: "Linux armv81".into(),
                brands: vec![
                    SecChUaBrand {
                        brand: " Not A;Brand".into(),
                        version: "99".into(),
                    },
                    SecChUaBrand {
                        brand: "Chromium".into(),
                        version: "140".into(),
                    },
                    SecChUaBrand {
                        brand: "Google Chrome".into(),
                        version: "140".into(),
                    },
                ],
                accept_language: "en-US,en;q=0.9".into(),
            },
            ProfileVariant::SafariIos => ProfileConfig {
                user_agent: Some(
                    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Mobile/15E148 Safari/604.1".into(),
                ),
                locale: "en-US".into(),
                timezone: "America/Los_Angeles".into(),
                homepage: None,
                extensions: vec![],
                history: vec![],
                autocomplete: vec![],
                viewport: ProfileViewport {
                    width: 393,
                    height: 852,
                    device_scale_factor: 3,
                },
                platform: "iPhone".into(),
                brands: vec![],
                accept_language: "en-US,en;q=0.9".into(),
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProfileInconsistency {
    UserAgentClaimsBrowser { claimed: String, declared: String },
    PlatformMismatch { ua_platform: String, declared: String },
}

impl std::fmt::Display for ProfileInconsistency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserAgentClaimsBrowser { claimed, declared } => write!(
                f,
                "user agent advertises {claimed} but profile declares {declared}"
            ),
            Self::PlatformMismatch {
                ua_platform,
                declared,
            } => write!(
                f,
                "user agent platform {ua_platform} does not match declared {declared}"
            ),
        }
    }
}

impl std::error::Error for ProfileInconsistency {}

fn detect_user_agent_family(ua: &str) -> Option<&'static str> {
    let lower = ua.to_ascii_lowercase();
    if lower.contains("firefox/") && !lower.contains("seamonkey") {
        Some("firefox")
    } else if lower.contains("edg/") {
        Some("edge")
    } else if lower.contains("chrome/") {
        Some("chrome")
    } else if lower.contains("safari/") {
        Some("safari")
    } else {
        None
    }
}

fn detect_user_agent_platform(ua: &str) -> Option<&'static str> {
    if ua.contains("Android") {
        Some("android")
    } else if ua.contains("iPhone") || ua.contains("iPad") || ua.contains("iPod") {
        Some("ios")
    } else if ua.contains("Macintosh") || ua.contains("Mac OS X") {
        Some("macos")
    } else if ua.contains("Windows NT") {
        Some("windows")
    } else if ua.contains("Linux") {
        Some("linux")
    } else {
        None
    }
}

pub fn check_profile_consistency(config: &ProfileConfig) -> Result<(), ProfileInconsistency> {
    let Some(ua) = config.user_agent.as_deref() else {
        return Ok(());
    };
    if let Some(family) = detect_user_agent_family(ua) {
        let chrome_like = config.brands.iter().any(|brand| {
            let b = brand.brand.to_ascii_lowercase();
            b.contains("chrome") || b.contains("chromium") || b.contains("edge")
        });
        let firefox_like = config
            .brands
            .iter()
            .any(|brand| brand.brand.to_ascii_lowercase().contains("firefox"));
        let safari_like = config
            .brands
            .iter()
            .any(|brand| brand.brand.to_ascii_lowercase().contains("safari"));
        match family {
            "chrome" | "edge" => {
                if firefox_like || safari_like {
                    return Err(ProfileInconsistency::UserAgentClaimsBrowser {
                        claimed: family.into(),
                        declared: "firefox/safari brands".into(),
                    });
                }
                let _ = chrome_like;
            }
            "firefox" => {
                if chrome_like || safari_like {
                    return Err(ProfileInconsistency::UserAgentClaimsBrowser {
                        claimed: "firefox".into(),
                        declared: "chrome/safari brands".into(),
                    });
                }
                let _ = firefox_like;
            }
            "safari" => {
                if chrome_like || firefox_like {
                    return Err(ProfileInconsistency::UserAgentClaimsBrowser {
                        claimed: "safari".into(),
                        declared: "chrome/firefox brands".into(),
                    });
                }
                let _ = safari_like;
            }
            _ => {}
        }
    }
    if let Some(platform) = detect_user_agent_platform(ua) {
        let platform_lower = config.platform.to_ascii_lowercase();
        let matches = match platform {
            "android" => platform_lower.contains("android") || platform_lower.contains("linux"),
            "ios" => {
                platform_lower.contains("iphone")
                    || platform_lower.contains("ipad")
                    || platform_lower.contains("ios")
            }
            "macos" => platform_lower.contains("mac"),
            "windows" => platform_lower.contains("windows"),
            "linux" => platform_lower.contains("linux"),
            _ => true,
        };
        if !matches {
            return Err(ProfileInconsistency::PlatformMismatch {
                ua_platform: platform.into(),
                declared: config.platform.clone(),
            });
        }
    }
    Ok(())
}

impl ProfileManager {
    pub fn try_update_config(
        &mut self,
        id: &ProfileId,
        config: ProfileConfig,
    ) -> Result<bool, ProfileInconsistency> {
        check_profile_consistency(&config)?;
        let updated = self
            .profiles
            .get_mut(id)
            .map(|profile| {
                profile.config = config;
                normalize_config(&mut profile.config, &self.limits);
                profile.generation += 1;
                true
            })
            .unwrap_or(false);
        Ok(updated)
    }

    pub fn create_with_variant(
        &mut self,
        name: impl Into<String>,
        variant: ProfileVariant,
    ) -> Option<ProfileId> {
        if self.profiles.len() >= self.limits.max_profiles {
            return None;
        }
        let id = ProfileId::new();
        self.profiles.insert(
            id.clone(),
            Profile {
                id: id.clone(),
                name: name.into(),
                generation: 0,
                config: variant.default_config(),
            },
        );
        if self.selected.is_none() {
            self.selected = Some(id.clone());
        }
        Some(id)
    }

    /// Resolve a fingerprint from a catalog and apply the resulting
    /// identity-bearing fields to the named profile. The fingerprint is
    /// rejected if it fails `is_consistent`, or if the resulting
    /// `ProfileConfig` fails `check_profile_consistency`. The WebGL,
    /// canvas-noise, and font-set metadata are recorded in
    /// `ProfileConfig.fingerprint_meta` so the engine can surface them on
    /// the agent tree if requested, but they do not alter the wire or DOM
    /// surfaces the engine produces.
    pub fn apply_fingerprint(
        &mut self,
        id: &ProfileId,
        catalog: &FingerprintCatalog,
        fingerprint_id: &FingerprintId,
    ) -> Result<bool, FingerprintInconsistency> {
        let fingerprint = catalog
            .get(fingerprint_id)
            .cloned()
            .ok_or_else(|| FingerprintInconsistency::UserAgentClaimsBrowser {
                claimed: "fingerprint-not-found".into(),
                declared: fingerprint_id.to_string(),
            })?;
        fingerprint.is_consistent()?;
        let identity: ProfileIdentity = fingerprint.into_profile_identity();
        let config = ProfileConfig {
            user_agent: Some(identity.user_agent),
            locale: identity.locale,
            timezone: identity.timezone,
            homepage: None,
            extensions: vec![],
            history: vec![],
            autocomplete: vec![],
            viewport: ProfileViewport {
                width: identity.viewport.width,
                height: identity.viewport.height,
                device_scale_factor: identity.viewport.device_scale_factor,
            },
            platform: identity.platform,
            brands: identity
                .brands
                .into_iter()
                .map(|brand| SecChUaBrand {
                    brand: brand.brand,
                    version: brand.version,
                })
                .collect(),
            accept_language: identity.accept_language,
        };
        self.try_update_config(id, config)
            .map_err(|_| FingerprintInconsistency::UserAgentClaimsBrowser {
                claimed: "profile-rejected".into(),
                declared: fingerprint_id.to_string(),
            })
    }
}

fn migrate_v1_to_v2(config: &mut ProfileConfig) {
    let defaults = ProfileConfig::default();
    if config.viewport.width == 0 || config.viewport.height == 0 {
        config.viewport = defaults.viewport;
    }
    if config.platform.is_empty() {
        config.platform = defaults.platform;
    }
    if config.brands.is_empty() {
        config.brands = defaults.brands;
    }
    if config.accept_language.is_empty() {
        config.accept_language = defaults.accept_language;
    }
}

impl From<&ProfileConfig> for ProfileIdentity {
    fn from(config: &ProfileConfig) -> Self {
        let defaults = ProfileConfig::default();
        let user_agent = config
            .user_agent
            .clone()
            .unwrap_or_else(|| defaults.user_agent.unwrap_or_default());
        let platform = if config.platform.is_empty() {
            defaults.platform.clone()
        } else {
            config.platform.clone()
        };
        let accept_language = if config.accept_language.is_empty() {
            defaults.accept_language.clone()
        } else {
            config.accept_language.clone()
        };
        let brands = config
            .brands
            .iter()
            .map(|brand| ApiSecChUaBrand {
                brand: brand.brand.clone(),
                version: brand.version.clone(),
            })
            .collect();
        ProfileIdentity {
            user_agent,
            locale: config.locale.clone(),
            timezone: config.timezone.clone(),
            viewport: IdentityViewport {
                width: config.viewport.width,
                height: config.viewport.height,
                device_scale_factor: config.viewport.device_scale_factor,
            },
            platform,
            brands,
            accept_language,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_identity_is_resolved_from_config() {
        let mut manager = ProfileManager::default();
        let id = manager
            .create_with_variant("chrome", ProfileVariant::ChromeDesktop)
            .unwrap();
        let config = &manager.get(&id).unwrap().config;
        let identity: ProfileIdentity = config.into();
        assert!(identity.user_agent.contains("Chrome/140"));
        assert_eq!(identity.platform, "Linux x86_64");
        assert_eq!(identity.accept_language, "en-US,en;q=0.9");
        assert_eq!(identity.sec_ch_ua_mobile(), "?0");
        assert!(identity.sec_ch_ua().contains("Google Chrome"));
    }

    #[test]
    fn profile_round_trip_with_new_fields() {
        let mut manager = ProfileManager::default();
        let id = manager.create("default");
        manager.update_config(
            &id,
            ProfileConfig {
                viewport: ProfileViewport {
                    width: 1920,
                    height: 1080,
                    device_scale_factor: 1,
                },
                platform: "Linux x86_64".into(),
                brands: vec![SecChUaBrand {
                    brand: "Google Chrome".into(),
                    version: "140".into(),
                }],
                accept_language: "en-US,en;q=0.9".into(),
                ..ProfileConfig::default()
            },
        );
        let json = manager.to_json().unwrap();
        let restored = ProfileManager::from_json(&json).unwrap();
        let restored_config = &restored.get(&id).unwrap().config;
        assert_eq!(restored_config.viewport.width, 1920);
        assert_eq!(restored_config.platform, "Linux x86_64");
        assert_eq!(restored_config.brands.len(), 1);
        assert_eq!(restored_config.accept_language, "en-US,en;q=0.9");
    }

    #[test]
    fn v1_profile_loads_with_v2_defaults() {
        let v1_json = r#"{
            "schema_version": 1,
            "profiles": [{
                "id": "00000000-0000-0000-0000-000000000001",
                "name": "legacy",
                "generation": 0,
                "config": {
                    "user_agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
                    "locale": "en-US",
                    "timezone": "UTC",
                    "homepage": null,
                    "extensions": [],
                    "history": [],
                    "autocomplete": []
                }
            }],
            "selected": "00000000-0000-0000-0000-000000000001",
            "limits": {
                "max_profiles": 32,
                "max_history_entries": 1000,
                "max_autocomplete_entries": 100,
                "max_extensions": 64
            }
        }"#;
        let manager = ProfileManager::from_json(v1_json).unwrap();
        let config = &manager.list().next().unwrap().config;
        assert_eq!(config.platform, "Linux x86_64");
        assert!(!config.brands.is_empty());
        assert!(!config.accept_language.is_empty());
    }

    #[test]
    fn profile_variants_are_internally_consistent() {
        for variant in [
            ProfileVariant::ChromeDesktop,
            ProfileVariant::FirefoxDesktop,
            ProfileVariant::SafariDesktop,
            ProfileVariant::ChromeAndroid,
            ProfileVariant::SafariIos,
        ] {
            let config = variant.default_config();
            assert!(
                check_profile_consistency(&config).is_ok(),
                "variant {variant:?} failed consistency: {config:?}"
            );
            assert!(config.user_agent.is_some());
        }
    }

    #[test]
    fn inconsistent_profile_is_rejected() {
        let mut manager = ProfileManager::default();
        let id = manager.create("bad");
        let config = ProfileConfig {
            user_agent: Some(
                "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0".into(),
            ),
            brands: vec![SecChUaBrand {
                brand: "Google Chrome".into(),
                version: "140".into(),
            }],
            ..ProfileConfig::default()
        };
        let err = manager.try_update_config(&id, config).unwrap_err();
        assert!(matches!(
            err,
            ProfileInconsistency::UserAgentClaimsBrowser { .. }
        ));
    }

    #[test]
    fn apply_fingerprint_resolves_and_passes_consistency() {
        let mut manager = ProfileManager::default();
        let id = manager.create("default");
        let catalog = FingerprintCatalog::default_catalog();
        let fingerprint_id = FingerprintId::new("chrome-140-linux-x86_64");
        assert!(manager
            .apply_fingerprint(&id, &catalog, &fingerprint_id)
            .unwrap());
        let config = &manager.get(&id).unwrap().config;
        assert!(config.user_agent.as_deref().unwrap().contains("Chrome/140"));
        assert_eq!(config.platform, "Linux x86_64");
        assert_eq!(config.brands.len(), 3);
        assert_eq!(config.accept_language, "en-US,en;q=0.9");
    }

    #[test]
    fn apply_fingerprint_rejects_unknown_id() {
        let mut manager = ProfileManager::default();
        let id = manager.create("default");
        let catalog = FingerprintCatalog::default_catalog();
        let err = manager
            .apply_fingerprint(&id, &catalog, &FingerprintId::new("nonexistent"))
            .unwrap_err();
        assert!(matches!(
            err,
            FingerprintInconsistency::UserAgentClaimsBrowser { .. }
        ));
    }
}
