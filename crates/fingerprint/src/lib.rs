//! Coherent browser-identity fingerprints.
//!
//! A [`Fingerprint`] bundles every identity-bearing value the engine exposes
//! over the wire and to JavaScript: the `User-Agent` string, the `Sec-CH-UA`
//! brand list, locale, timezone, viewport, platform, `Accept-Language`, WebGL
//! vendor/renderer, and the font set the browser reports as available.
//!
//! This crate is **passive**. It does not implement TLS fingerprint spoofing,
//! residential-proxy rotation, behavior obfuscation, or any other anti-bot
//! evasion technique. The fingerprint is metadata that the engine uses to
//! keep its reported surfaces consistent; it never tampers with the values
//! the browser would otherwise produce. See `docs/browser-fidelity.md` for
//! the explicit "out of scope" list.

use browsai_engine_api::{IdentityViewport, ProfileIdentity, SecChUaBrand};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FingerprintId(String);

impl FingerprintId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FingerprintId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for FingerprintId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub id: FingerprintId,
    pub user_agent: String,
    pub locale: String,
    pub timezone: String,
    pub viewport: IdentityViewport,
    pub platform: String,
    pub brands: Vec<SecChUaBrand>,
    pub accept_language: String,
    pub webgl_vendor: String,
    pub webgl_renderer: String,
    pub canvas_noise_seed: u32,
    pub font_set: Vec<String>,
}

impl Fingerprint {
    pub fn into_profile_identity(self) -> ProfileIdentity {
        ProfileIdentity {
            user_agent: self.user_agent,
            locale: self.locale,
            timezone: self.timezone,
            viewport: self.viewport,
            platform: self.platform,
            brands: self.brands,
            accept_language: self.accept_language,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FingerprintInconsistency {
    UserAgentClaimsBrowser {
        claimed: String,
        declared: String,
    },
    PlatformMismatch {
        ua_platform: String,
        declared: String,
    },
    WebglMismatch {
        ua_family: String,
        webgl_vendor: String,
    },
}

impl std::fmt::Display for FingerprintInconsistency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserAgentClaimsBrowser { claimed, declared } => write!(
                f,
                "user agent advertises {claimed} but fingerprint declares {declared}"
            ),
            Self::PlatformMismatch {
                ua_platform,
                declared,
            } => write!(
                f,
                "user agent platform {ua_platform} does not match declared {declared}"
            ),
            Self::WebglMismatch {
                ua_family,
                webgl_vendor,
            } => write!(
                f,
                "WebGL vendor {webgl_vendor} does not match {ua_family} browser conventions"
            ),
        }
    }
}

impl std::error::Error for FingerprintInconsistency {}

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

impl Fingerprint {
    pub fn is_consistent(&self) -> Result<(), FingerprintInconsistency> {
        if let Some(family) = detect_user_agent_family(&self.user_agent) {
            let chrome_like = self.brands.iter().any(|brand| {
                let b = brand.brand.to_ascii_lowercase();
                b.contains("chrome") || b.contains("chromium") || b.contains("edge")
            });
            let firefox_like = self
                .brands
                .iter()
                .any(|brand| brand.brand.to_ascii_lowercase().contains("firefox"));
            let safari_like = self
                .brands
                .iter()
                .any(|brand| brand.brand.to_ascii_lowercase().contains("safari"));
            match family {
                "chrome" | "edge" => {
                    if firefox_like || safari_like {
                        return Err(FingerprintInconsistency::UserAgentClaimsBrowser {
                            claimed: family.into(),
                            declared: "firefox/safari brands".into(),
                        });
                    }
                }
                "firefox" => {
                    if chrome_like || safari_like {
                        return Err(FingerprintInconsistency::UserAgentClaimsBrowser {
                            claimed: "firefox".into(),
                            declared: "chrome/safari brands".into(),
                        });
                    }
                }
                "safari" if chrome_like || firefox_like => {
                    return Err(FingerprintInconsistency::UserAgentClaimsBrowser {
                        claimed: "safari".into(),
                        declared: "chrome/firefox brands".into(),
                    });
                }
                _ => {}
            }
        }
        if let Some(platform) = detect_user_agent_platform(&self.user_agent) {
            let platform_lower = self.platform.to_ascii_lowercase();
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
                return Err(FingerprintInconsistency::PlatformMismatch {
                    ua_platform: platform.into(),
                    declared: self.platform.clone(),
                });
            }
        }
        if let Some(family) = detect_user_agent_family(&self.user_agent) {
            let vendor_lower = self.webgl_vendor.to_ascii_lowercase();
            let renderer_lower = self.webgl_renderer.to_ascii_lowercase();
            let expects_chrome = matches!(family, "chrome" | "edge");
            let chrome_marker = vendor_lower.contains("google")
                || renderer_lower.contains("angle")
                || vendor_lower.contains("intel")
                || renderer_lower.contains("mesa")
                || renderer_lower.contains("apple");
            if expects_chrome && !chrome_marker && !vendor_lower.is_empty() {
                return Err(FingerprintInconsistency::WebglMismatch {
                    ua_family: family.into(),
                    webgl_vendor: self.webgl_vendor.clone(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FingerprintCatalog {
    entries: BTreeMap<FingerprintId, Fingerprint>,
}

impl FingerprintCatalog {
    pub fn default_catalog() -> Self {
        let mut catalog = Self::default();
        for entry in default_catalog_entries() {
            catalog.entries.insert(entry.id.clone(), entry);
        }
        catalog
    }

    pub fn get(&self, id: &FingerprintId) -> Option<&Fingerprint> {
        self.entries.get(id)
    }

    pub fn insert(&mut self, fingerprint: Fingerprint) -> Result<(), FingerprintInconsistency> {
        fingerprint.is_consistent()?;
        self.entries.insert(fingerprint.id.clone(), fingerprint);
        Ok(())
    }

    pub fn list(&self) -> impl Iterator<Item = &Fingerprint> {
        self.entries.values()
    }

    pub fn ids(&self) -> impl Iterator<Item = &FingerprintId> {
        self.entries.keys()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn default_catalog_entries() -> Vec<Fingerprint> {
    vec![
        Fingerprint {
            id: FingerprintId::new("chrome-140-linux-x86_64"),
            user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36".into(),
            locale: "en-US".into(),
            timezone: "America/Los_Angeles".into(),
            viewport: IdentityViewport { width: 1920, height: 1080, device_scale_factor: 1 },
            platform: "Linux x86_64".into(),
            brands: vec![
                SecChUaBrand { brand: " Not A;Brand".into(), version: "99".into() },
                SecChUaBrand { brand: "Chromium".into(), version: "140".into() },
                SecChUaBrand { brand: "Google Chrome".into(), version: "140".into() },
            ],
            accept_language: "en-US,en;q=0.9".into(),
            webgl_vendor: "Google Inc. (Intel)".into(),
            webgl_renderer: "ANGLE (Intel, Mesa Intel(R) UHD Graphics 620 (KBL GT2), OpenGL 4.5)".into(),
            canvas_noise_seed: 0,
            font_set: vec![
                "Andale Mono".into(),
                "Arial".into(),
                "Arial Black".into(),
                "Comic Sans MS".into(),
                "Courier".into(),
                "Courier New".into(),
                "DejaVu Sans".into(),
                "Georgia".into(),
                "Helvetica".into(),
                "Impact".into(),
                "Liberation Sans".into(),
                "Noto Sans".into(),
                "Times".into(),
                "Times New Roman".into(),
                "Trebuchet MS".into(),
                "Verdana".into(),
            ],
        },
        Fingerprint {
            id: FingerprintId::new("firefox-130-linux-x86_64"),
            user_agent: "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0".into(),
            locale: "en-US".into(),
            timezone: "America/Los_Angeles".into(),
            viewport: IdentityViewport { width: 1920, height: 1080, device_scale_factor: 1 },
            platform: "Linux x86_64".into(),
            brands: vec![],
            accept_language: "en-US,en;q=0.5".into(),
            webgl_vendor: "Mozilla".into(),
            webgl_renderer: "Mesa Intel(R) UHD Graphics 620 (KBL GT2)".into(),
            canvas_noise_seed: 0,
            font_set: vec![
                "Andale Mono".into(),
                "Arial".into(),
                "Arial Black".into(),
                "Comic Sans MS".into(),
                "Courier".into(),
                "Courier New".into(),
                "DejaVu Sans".into(),
                "Georgia".into(),
                "Impact".into(),
                "Liberation Sans".into(),
                "Noto Sans".into(),
                "Times".into(),
                "Times New Roman".into(),
                "Trebuchet MS".into(),
                "Verdana".into(),
            ],
        },
        Fingerprint {
            id: FingerprintId::new("safari-17-macos-arm64"),
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_6) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15".into(),
            locale: "en-US".into(),
            timezone: "America/Los_Angeles".into(),
            viewport: IdentityViewport { width: 2560, height: 1440, device_scale_factor: 2 },
            platform: "MacIntel".into(),
            brands: vec![],
            accept_language: "en-US,en;q=0.9".into(),
            webgl_vendor: "WebKit Inc.".into(),
            webgl_renderer: "Apple M1 Pro".into(),
            canvas_noise_seed: 0,
            font_set: vec![
                "American Typewriter".into(),
                "Avenir".into(),
                "Avenir Next".into(),
                "Courier".into(),
                "Courier New".into(),
                "Geneva".into(),
                "Georgia".into(),
                "Helvetica".into(),
                "Helvetica Neue".into(),
                "Impact".into(),
                "Lucida Grande".into(),
                "Monaco".into(),
                "Optima".into(),
                "Palatino".into(),
                "Times".into(),
                "Times New Roman".into(),
                "Verdana".into(),
            ],
        },
        Fingerprint {
            id: FingerprintId::new("chrome-android-140-pixel8"),
            user_agent: "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36".into(),
            locale: "en-US".into(),
            timezone: "America/Los_Angeles".into(),
            viewport: IdentityViewport { width: 412, height: 915, device_scale_factor: 2 },
            platform: "Linux armv81".into(),
            brands: vec![
                SecChUaBrand { brand: " Not A;Brand".into(), version: "99".into() },
                SecChUaBrand { brand: "Chromium".into(), version: "140".into() },
                SecChUaBrand { brand: "Google Chrome".into(), version: "140".into() },
            ],
            accept_language: "en-US,en;q=0.9".into(),
            webgl_vendor: "Google Inc. (ARM)".into(),
            webgl_renderer: "ANGLE (ARM, Mali-G715, OpenGL 4.5)".into(),
            canvas_noise_seed: 0,
            font_set: vec![
                "Roboto".into(),
                "Roboto Condensed".into(),
                "Roboto Mono".into(),
                "Noto Sans".into(),
                "Noto Sans CJK".into(),
                "Noto Color Emoji".into(),
                "Droid Sans".into(),
            ],
        },
        Fingerprint {
            id: FingerprintId::new("safari-ios-17-iphone"),
            user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Mobile/15E148 Safari/604.1".into(),
            locale: "en-US".into(),
            timezone: "America/Los_Angeles".into(),
            viewport: IdentityViewport { width: 393, height: 852, device_scale_factor: 3 },
            platform: "iPhone".into(),
            brands: vec![],
            accept_language: "en-US,en;q=0.9".into(),
            webgl_vendor: "WebKit Inc.".into(),
            webgl_renderer: "Apple A16 GPU".into(),
            canvas_noise_seed: 0,
            font_set: vec![
                "Helvetica Neue".into(),
                "Helvetica".into(),
                "Arial".into(),
                "Courier".into(),
                "Courier New".into(),
                "Georgia".into(),
                "Times".into(),
                "Times New Roman".into(),
                "Verdana".into(),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_entries_are_internally_consistent() {
        let catalog = FingerprintCatalog::default_catalog();
        assert!(catalog.len() >= 5);
        for fingerprint in catalog.list() {
            fingerprint
                .is_consistent()
                .unwrap_or_else(|err| panic!("fingerprint {} failed: {err}", fingerprint.id));
        }
    }

    #[test]
    fn inconsistent_fingerprint_is_rejected_on_insert() {
        let mut catalog = FingerprintCatalog::default_catalog();
        let bad = Fingerprint {
            id: FingerprintId::new("chrome-with-firefox-brands"),
            user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36".into(),
            locale: "en-US".into(),
            timezone: "UTC".into(),
            viewport: IdentityViewport { width: 1280, height: 720, device_scale_factor: 1 },
            platform: "Linux x86_64".into(),
            brands: vec![SecChUaBrand { brand: "Firefox".into(), version: "130".into() }],
            accept_language: "en-US,en;q=0.9".into(),
            webgl_vendor: "Google Inc. (Intel)".into(),
            webgl_renderer: "ANGLE".into(),
            canvas_noise_seed: 0,
            font_set: vec![],
        };
        assert!(matches!(
            catalog.insert(bad),
            Err(FingerprintInconsistency::UserAgentClaimsBrowser { .. })
        ));
    }

    #[test]
    fn platform_mismatch_is_detected() {
        let bad = Fingerprint {
            id: FingerprintId::new("android-pretending-windows"),
            user_agent: "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36".into(),
            locale: "en-US".into(),
            timezone: "UTC".into(),
            viewport: IdentityViewport { width: 412, height: 915, device_scale_factor: 2 },
            platform: "Windows NT 10.0".into(),
            brands: vec![SecChUaBrand { brand: "Google Chrome".into(), version: "140".into() }],
            accept_language: "en-US".into(),
            webgl_vendor: "Google Inc. (ARM)".into(),
            webgl_renderer: "ANGLE".into(),
            canvas_noise_seed: 0,
            font_set: vec![],
        };
        assert!(matches!(
            bad.is_consistent(),
            Err(FingerprintInconsistency::PlatformMismatch { .. })
        ));
    }

    #[test]
    fn into_profile_identity_drops_fingerprint_metadata() {
        let fingerprint = FingerprintCatalog::default_catalog()
            .get(&FingerprintId::new("chrome-140-linux-x86_64"))
            .cloned()
            .unwrap();
        let identity: ProfileIdentity = fingerprint.into_profile_identity();
        assert!(identity.user_agent.contains("Chrome/140"));
        assert_eq!(identity.platform, "Linux x86_64");
        assert_eq!(identity.brands.len(), 3);
    }
}
