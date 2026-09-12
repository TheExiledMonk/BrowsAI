use browsai_input::NativeInputEvent;
use browsai_state::{PageSnapshot, PageState};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

pub type ContextId = u64;
pub type PageId = u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IdentityViewport {
    pub width: u32,
    pub height: u32,
    pub device_scale_factor: u32,
}

impl Default for IdentityViewport {
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
pub struct ProfileIdentity {
    pub user_agent: String,
    pub locale: String,
    pub timezone: String,
    pub viewport: IdentityViewport,
    pub platform: String,
    pub brands: Vec<SecChUaBrand>,
    pub accept_language: String,
    /// `navigator.hardwareConcurrency`. Typical: 2, 4, 8, 16.
    pub hardware_concurrency: u8,
    /// `navigator.deviceMemory` in GB. Typical: 2, 4, 8, 16.
    pub device_memory: u8,
    /// `navigator.maxTouchPoints`. 0 on desktop, 5+ on touch devices.
    pub max_touch_points: u8,
    /// `screen.colorDepth`. 24 standard, 30 for HDR.
    pub color_depth: u8,
}

impl ProfileIdentity {
    pub fn default_for_servo() -> Self {
        Self {
            user_agent:
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36"
                    .into(),
            locale: "en-US".into(),
            timezone: "UTC".into(),
            viewport: IdentityViewport::default(),
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
            hardware_concurrency: 8,
            device_memory: 8,
            max_touch_points: 0,
            color_depth: 24,
        }
    }

    pub fn sec_ch_ua(&self) -> String {
        self.brands
            .iter()
            .map(|brand| {
                let escaped = brand.brand.replace('\u{0000}', "");
                format!("\"{escaped}\";v=\"{}\"", brand.version)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn sec_ch_ua_mobile(&self) -> &'static str {
        if self.platform.to_ascii_lowercase().contains("android")
            || self.platform.to_ascii_lowercase().contains("iphone")
            || self.platform.to_ascii_lowercase().contains("ipad")
        {
            "?1"
        } else {
            "?0"
        }
    }

    /// `true` when the platform string carries an Android/iOS/iPadOS marker,
    /// which is the most reliable UA-side signal even though Android Chrome
    /// reports `navigator.platform` as `Linux armv8l`.
    pub fn ua_claims_touch(&self) -> bool {
        let platform = self.platform.to_ascii_lowercase();
        platform.contains("android") || platform.contains("iphone") || platform.contains("ipad")
    }

    pub fn sec_ch_ua_platform(&self) -> String {
        format!("\"{}\"", self.platform)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ContextOptions {
    pub profile: Option<String>,
    pub profile_identity: Option<ProfileIdentity>,
    pub headless: bool,
    pub use_real_browser_runtime: bool,
    pub viewport: Option<VirtualViewport>,
    pub deterministic_clock_millis: Option<u64>,
    pub no_raster: bool,
    /// HTTP/2 SETTINGS profile for the live Servo runtime's HTTP client.
    /// `None` keeps hyper-util's defaults. The vendored `servo-net`
    /// connector applies real-browser values for Firefox-130, Chrome-140,
    /// and Edge when one of these is selected. See `docs/fingerprint-hardening.md`
    /// for the per-profile knob tables.
    pub http2_profile: Option<Http2Profile>,
}

/// HTTP/2 SETTINGS profile presets. Each maps to the knobs measured from
/// fresh sessions of the named browser. `Default` (None in ContextOptions)
/// keeps hyper-util's stock defaults; this enum is opt-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Http2Profile {
    Firefox130,
    Chrome140,
    Edge,
}

impl Http2Profile {
    /// Render as a stable string for CLI / capabilities output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Http2Profile::Firefox130 => "firefox-130",
            Http2Profile::Chrome140 => "chrome-140",
            Http2Profile::Edge => "edge",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VirtualViewport {
    pub width: u32,
    pub height: u32,
    pub device_scale_factor: f32,
}

impl Default for VirtualViewport {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            device_scale_factor: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScriptSource(pub String);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageEvaluationRequest {
    pub source: ScriptSource,
    pub origin: String,
    pub capability_granted: bool,
    pub timeout_millis: u64,
    pub max_timeout_millis: u64,
    pub provenance_reference: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PageScriptResult {
    pub value: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum EngineFeature {
    Navigation,
    Snapshots,
    NativeInput,
    PageEvaluation,
    NetworkObservation,
    LayoutObservation,
    RuntimeObservation,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EngineCapabilities {
    pub engine_name: String,
    pub engine_version: Option<String>,
    pub features: std::collections::BTreeSet<EngineFeature>,
    /// True when the engine was compiled with the live-runtime feature and
    /// can drive a real browser (e.g. Servo embedder) rather than only the
    /// deterministic backend.
    pub live_browser_compiled: bool,
    /// Command surface the host shell can introspect (used by the CLI's
    /// `capabilities` command). Each entry includes the subcommand name,
    /// the optional flag list, a short description, and an example payload
    /// the host can show or test against.
    pub commands: Vec<EngineCommand>,
}

/// One entry in `EngineCapabilities::commands`. Schemas the host shell
/// can introspect at runtime instead of hard-coding CLI invocations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngineCommand {
    pub name: String,
    pub args: Vec<String>,
    pub returns: String,
    pub example: Option<serde_json::Value>,
}

impl EngineCapabilities {
    pub fn supports(&self, feature: EngineFeature) -> bool {
        self.features.contains(&feature)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NavigationHandle {
    pub page: PageId,
    pub url: Url,
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("context not found: {0}")]
    ContextNotFound(ContextId),
    #[error("page not found: {0}")]
    PageNotFound(PageId),
    #[error("operation is unsupported by this engine adapter: {0}")]
    Unsupported(String),
    #[error("engine error: {0}")]
    Other(String),
    #[error("page evaluation capability denied")]
    EvaluationDenied,
    #[error("page evaluation timeout exceeds policy")]
    EvaluationTimeout,
    #[error("page evaluation provenance is required")]
    MissingProvenance,
}

pub trait BrowserEngine {
    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities::default()
    }
    fn create_context(&mut self, options: ContextOptions) -> Result<ContextId, EngineError>;
    fn create_page(&mut self, context: ContextId) -> Result<PageId, EngineError>;
    fn navigate(&mut self, page: PageId, url: Url) -> Result<NavigationHandle, EngineError>;
    fn snapshot(&self, page: PageId) -> Result<PageSnapshot, EngineError>;
    fn page_state(&self, page: PageId) -> Result<PageState, EngineError>;
    fn dispatch_input(&mut self, page: PageId, event: NativeInputEvent) -> Result<(), EngineError>;
    fn evaluate_page_script(
        &mut self,
        page: PageId,
        source: ScriptSource,
    ) -> Result<PageScriptResult, EngineError>;

    fn advance_time(&mut self, _context: ContextId, _millis: u64) -> Result<u64, EngineError> {
        Err(EngineError::Unsupported(
            "deterministic clock is unavailable for this context".into(),
        ))
    }

    fn evaluate_page_script_checked(
        &mut self,
        page: PageId,
        request: PageEvaluationRequest,
    ) -> Result<PageScriptResult, EngineError> {
        if request.origin.is_empty() || !request.capability_granted {
            return Err(EngineError::EvaluationDenied);
        }
        if request.provenance_reference.is_empty() {
            return Err(EngineError::MissingProvenance);
        }
        if request.timeout_millis == 0 || request.timeout_millis > request.max_timeout_millis {
            return Err(EngineError::EvaluationTimeout);
        }
        self.evaluate_page_script(page, request.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http2_profile_as_str_round_trips() {
        assert_eq!(Http2Profile::Firefox130.as_str(), "firefox-130");
        assert_eq!(Http2Profile::Chrome140.as_str(), "chrome-140");
        assert_eq!(Http2Profile::Edge.as_str(), "edge");
    }

    #[test]
    fn http2_profile_distinct_variants() {
        // Each profile must be a distinct variant so the CLI flag round-trips.
        assert_ne!(Http2Profile::Firefox130, Http2Profile::Chrome140);
        assert_ne!(Http2Profile::Firefox130, Http2Profile::Edge);
        assert_ne!(Http2Profile::Chrome140, Http2Profile::Edge);
    }

    #[test]
    fn context_options_default_http2_profile_is_none() {
        let opts = ContextOptions::default();
        assert_eq!(opts.http2_profile, None);
    }
}
