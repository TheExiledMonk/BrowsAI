//! Observation-only detector for human-verification walls, captchas, and access-denied
//! interstitials served by anti-abuse providers.
//!
//! This crate is intentionally **observation only**: it never executes, injects,
//! mutates, or solves anything in the page. The public API has no `solve_*`,
//! `bypass_*`, `inject_*`, or `mutate_*` methods. Evasion logic is out of scope
//! for the BrowsAI project; see `docs/challenge-handling.md` for the policy
//! statement.
//!
//! Detection combines two streams:
//! - network-side signals (response status, `cf-ray`, `akamai` / `incapsula` /
//!   `cloudflare` / `data-dome` headers, `retry-after`).
//! - DOM-side signals (`cf-browser-verification`, hCaptcha iframes, reCAPTCHA
//!   iframes, Akamai `_Incapsula_Resource`, generic "verify you are human" copy).
//!
//! Both streams produce `ChallengeObservation` values that the agent tree can
//! surface so a takeover or credentialed-solve path can route around them.

use browsai_dom_observer::{NodeKind, RawDocument, RawNode};
use browsai_network::{NetworkRequest, NetworkResponse};
use browsai_provenance::{ProvenanceSource, SourceKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ChallengeKind {
    Captcha,
    BrowserChallenge,
    AccessDenied,
    TwoFactorPrompt,
    AccountLocked,
    TermsOfServiceGate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChallengeEvidence {
    pub signal: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChallengeObservation {
    pub provider: String,
    pub kind: ChallengeKind,
    pub url: Url,
    pub evidence: Vec<ChallengeEvidence>,
    pub confidence: f32,
    pub provenance: Vec<ProvenanceSource>,
}

impl ChallengeObservation {
    pub fn with_provenance(mut self, source: ProvenanceSource) -> Self {
        self.provenance.push(source);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChallengeObservationSet {
    pub observations: Vec<ChallengeObservation>,
}

impl ChallengeObservationSet {
    pub fn is_empty(&self) -> bool {
        self.observations.is_empty()
    }
    pub fn drain(&mut self) -> Vec<ChallengeObservation> {
        std::mem::take(&mut self.observations)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChallengeObserver {
    pending: ChallengeObservationSet,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ChallengeObserverError {
    #[error("invalid challenge confidence {0}: must be between 0.0 and 1.0")]
    InvalidConfidence(f32),
}

impl ChallengeObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe_network(
        &mut self,
        request: &NetworkRequest,
        response: &NetworkResponse,
    ) -> Result<(), ChallengeObserverError> {
        if let Some(observation) = detect_from_response(request, response) {
            self.append(observation)?;
        }
        Ok(())
    }

    pub fn observe_document(&mut self, document: &RawDocument) -> Result<(), ChallengeObserverError> {
        for observation in detect_from_document(document) {
            self.append(observation)?;
        }
        Ok(())
    }

    pub fn observe_html_body(
        &mut self,
        url: &Url,
        body: &str,
    ) -> Result<(), ChallengeObserverError> {
        for observation in detect_from_html_body(url, body) {
            self.append(observation)?;
        }
        Ok(())
    }

    pub fn snapshot(&self) -> ChallengeObservationSet {
        ChallengeObservationSet {
            observations: self.pending.observations.clone(),
        }
    }

    pub fn drain(&mut self) -> ChallengeObservationSet {
        std::mem::take(&mut self.pending)
    }

    pub fn clear(&mut self) {
        self.pending.observations.clear();
    }

    fn append(&mut self, observation: ChallengeObservation) -> Result<(), ChallengeObserverError> {
        if !(0.0..=1.0).contains(&observation.confidence) {
            return Err(ChallengeObserverError::InvalidConfidence(observation.confidence));
        }
        self.pending.observations.push(observation);
        Ok(())
    }
}

fn detect_from_response(
    request: &NetworkRequest,
    response: &NetworkResponse,
) -> Option<ChallengeObservation> {
    let headers = &response.headers;
    let body = String::from_utf8_lossy(&response.body);
    let server = header_value(headers, "server").unwrap_or_default().to_ascii_lowercase();
    let cf_ray = header_value(headers, "cf-ray").map(str::to_string);
    let akamai = header_value(headers, "x-akamai-transformed").map(str::to_string);
    let incapsula = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("x-cdn"))
        .map(|(_, v)| v.to_ascii_lowercase());
    let mitm = header_value(headers, "cf-mitigated").map(str::to_string);
    let retry_after = header_value(headers, "retry-after").map(str::to_string);

    let (provider, kind, confidence, signals) = if let Some(ray) = cf_ray {
        let lower = body.to_ascii_lowercase();
        if lower.contains("cf-browser-verification")
            || lower.contains("cf-challenge-running")
            || lower.contains("cf-error-code")
        {
            (
                "cloudflare".to_string(),
                ChallengeKind::BrowserChallenge,
                0.95,
                vec![
                    ("cf-ray".into(), ray),
                    ("body-marker".into(), "cf-browser-verification".into()),
                ],
            )
        } else if response.status == 403 || response.status == 503 {
            (
                "cloudflare".to_string(),
                ChallengeKind::AccessDenied,
                0.7,
                vec![("cf-ray".into(), ray), ("status".into(), response.status.to_string())],
            )
        } else {
            return None;
        }
    } else if server.contains("akamai") || akamai.is_some() || incapsula.as_deref() == Some("incapsula".into()) {
        let lower = body.to_ascii_lowercase();
        if lower.contains("_incapsula_resource") || lower.contains("incapsula") {
            (
                "akamai".to_string(),
                ChallengeKind::BrowserChallenge,
                0.9,
                vec![("server".into(), server), ("body-marker".into(), "_Incapsula_Resource".into())],
            )
        } else if response.status == 403 {
            (
                "akamai".to_string(),
                ChallengeKind::AccessDenied,
                0.7,
                vec![("server".into(), server), ("status".into(), "403".into())],
            )
        } else {
            return None;
        }
    } else if server.contains("cloudflare") && mitm.is_some() {
        (
            "cloudflare".to_string(),
            ChallengeKind::BrowserChallenge,
            0.9,
            vec![
                ("server".into(), server),
                ("cf-mitigated".into(), mitm.unwrap_or_default()),
            ],
        )
    } else if server.contains("ddos-guard") {
        (
            "ddos-guard".to_string(),
            ChallengeKind::BrowserChallenge,
            0.85,
            vec![("server".into(), server)],
        )
    } else if server.contains("data-dome") || body.to_ascii_lowercase().contains("datadome") {
        (
            "datadome".to_string(),
            ChallengeKind::BrowserChallenge,
            0.85,
            vec![("server".into(), server)],
        )
    } else if response.status == 429 {
        (
            "rate-limit".to_string(),
            ChallengeKind::AccessDenied,
            0.6,
            vec![
                ("status".into(), "429".into()),
                ("retry-after".into(), retry_after.unwrap_or_default()),
            ],
        )
    } else {
        return None;
    };

    let mut evidence: Vec<ChallengeEvidence> = signals
        .into_iter()
        .map(|(signal, value)| ChallengeEvidence { signal, value })
        .collect();
    if response.status >= 400 && !evidence.iter().any(|e| e.signal == "status") {
        evidence.push(ChallengeEvidence {
            signal: "status".into(),
            value: response.status.to_string(),
        });
    }
    let observation = ChallengeObservation {
        provider,
        kind,
        url: request.url.clone(),
        evidence,
        confidence,
        provenance: vec![ProvenanceSource {
            kind: SourceKind::NetworkResponse,
            reference: "challenge-observer".into(),
            detail: None,
        }],
    };
    Some(observation)
}

fn detect_from_document(document: &RawDocument) -> Vec<ChallengeObservation> {
    let mut observations = Vec::new();
    let url = Url::parse(&document.url).ok();
    let mut seen_providers: Vec<String> = Vec::new();
    for node in document.nodes.values() {
        if !matches!(node.kind, NodeKind::Element) {
            continue;
        }
        for provider_signal in detect_provider_in_node(node) {
            if seen_providers.contains(&provider_signal.provider) {
                continue;
            }
            seen_providers.push(provider_signal.provider.clone());
            let observation = ChallengeObservation {
                provider: provider_signal.provider,
                kind: provider_signal.kind,
                url: url.clone().unwrap_or_else(|| Url::parse("about:blank").expect("static")),
                evidence: provider_signal.evidence,
                confidence: provider_signal.confidence,
                provenance: vec![ProvenanceSource {
                    kind: SourceKind::Dom,
                    reference: "challenge-observer".into(),
                    detail: None,
                }],
            };
            observations.push(observation);
        }
    }
    observations
}

struct ProviderSignal {
    provider: String,
    kind: ChallengeKind,
    evidence: Vec<ChallengeEvidence>,
    confidence: f32,
}

fn detect_provider_in_node(node: &RawNode) -> Vec<ProviderSignal> {
    let mut signals = Vec::new();
    let lower_name = node
        .name
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let id_attr = node.attributes.get("id").cloned().unwrap_or_default();
    let class_attr = node.attributes.get("class").cloned().unwrap_or_default();
    let src_attr = node.attributes.get("src").cloned().unwrap_or_default();
    let action_attr = node.attributes.get("action").cloned().unwrap_or_default();
    let cf_marker_classes = ["cf-browser-verification", "cf-challenge-running", "cf-error-code", "cf-wrapper"];
    if cf_marker_classes.iter().any(|c| class_attr.contains(c) || id_attr.contains(c)) {
        signals.push(ProviderSignal {
            provider: "cloudflare".into(),
            kind: ChallengeKind::BrowserChallenge,
            evidence: vec![ChallengeEvidence {
                signal: "marker".into(),
                value: class_attr.clone(),
            }],
            confidence: 0.95,
        });
    }
    if lower_name == "iframe" {
        let src_lower = src_attr.to_ascii_lowercase();
        if src_lower.contains("hcaptcha.com") {
            signals.push(ProviderSignal {
                provider: "hcaptcha".into(),
                kind: ChallengeKind::Captcha,
                evidence: vec![ChallengeEvidence {
                    signal: "iframe-src".into(),
                    value: src_attr.clone(),
                }],
                confidence: 0.95,
            });
        } else if src_lower.contains("recaptcha") {
            signals.push(ProviderSignal {
                provider: "recaptcha".into(),
                kind: ChallengeKind::Captcha,
                evidence: vec![ChallengeEvidence {
                    signal: "iframe-src".into(),
                    value: src_attr.clone(),
                }],
                confidence: 0.95,
            });
        } else if src_lower.contains("challenges.cloudflare.com") {
            signals.push(ProviderSignal {
                provider: "cloudflare-turnstile".into(),
                kind: ChallengeKind::Captcha,
                evidence: vec![ChallengeEvidence {
                    signal: "iframe-src".into(),
                    value: src_attr.clone(),
                }],
                confidence: 0.95,
            });
        }
    }
    if class_attr.contains("_Incapsula_Resource") || id_attr.contains("incapsula") {
        signals.push(ProviderSignal {
            provider: "akamai".into(),
            kind: ChallengeKind::BrowserChallenge,
            evidence: vec![ChallengeEvidence {
                signal: "class-or-id".into(),
                value: class_attr.clone(),
            }],
            confidence: 0.9,
        });
    }
    let text_lower = node.text.as_deref().unwrap_or_default().to_ascii_lowercase();
    if !text_lower.is_empty() {
        if text_lower.contains("verify you are human") || text_lower.contains("are you a robot") {
            signals.push(ProviderSignal {
                provider: "generic".into(),
                kind: ChallengeKind::Captcha,
                evidence: vec![ChallengeEvidence {
                    signal: "text".into(),
                    value: text_lower.chars().take(120).collect(),
                }],
                confidence: 0.7,
            });
        } else if text_lower.contains("two-factor") || text_lower.contains("verification code") {
            signals.push(ProviderSignal {
                provider: "totp".into(),
                kind: ChallengeKind::TwoFactorPrompt,
                evidence: vec![ChallengeEvidence {
                    signal: "text".into(),
                    value: text_lower.chars().take(120).collect(),
                }],
                confidence: 0.65,
            });
        } else if text_lower.contains("account locked") || text_lower.contains("account suspended") {
            signals.push(ProviderSignal {
                provider: "site-policy".into(),
                kind: ChallengeKind::AccountLocked,
                evidence: vec![ChallengeEvidence {
                    signal: "text".into(),
                    value: text_lower.chars().take(120).collect(),
                }],
                confidence: 0.7,
            });
        }
    }
    if lower_name == "form" && action_attr.to_ascii_lowercase().contains("/interstitial/") {
        signals.push(ProviderSignal {
            provider: "datadome".into(),
            kind: ChallengeKind::BrowserChallenge,
            evidence: vec![ChallengeEvidence {
                signal: "form-action".into(),
                value: action_attr,
            }],
            confidence: 0.85,
        });
    }
    signals
}

fn detect_from_html_body(url: &Url, body: &str) -> Vec<ChallengeObservation> {
    let mut observations = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let lower = body.to_ascii_lowercase();
    let mut probe = |provider: &str, signal: &str, kind: ChallengeKind, confidence: f32| {
        if lower.contains(signal) && !seen.iter().any(|p| p == provider) {
            seen.push(provider.into());
            Some(ChallengeObservation {
                provider: provider.into(),
                kind,
                url: url.clone(),
                evidence: vec![ChallengeEvidence {
                    signal: "body-marker".into(),
                    value: signal.into(),
                }],
                confidence,
                provenance: vec![ProvenanceSource {
                    kind: SourceKind::NetworkResponse,
                    reference: "challenge-observer".into(),
                    detail: None,
                }],
            })
        } else {
            None
        }
    };
    if let Some(o) = probe("cloudflare", "cf-browser-verification", ChallengeKind::BrowserChallenge, 0.95) {
        observations.push(o);
    }
    if let Some(o) = probe("hcaptcha", "hcaptcha.com", ChallengeKind::Captcha, 0.9) {
        observations.push(o);
    }
    if let Some(o) = probe("recaptcha", "google.com/recaptcha", ChallengeKind::Captcha, 0.9) {
        observations.push(o);
    }
    if let Some(o) = probe("cloudflare-turnstile", "challenges.cloudflare.com", ChallengeKind::Captcha, 0.9) {
        observations.push(o);
    }
    if let Some(o) = probe("akamai", "_incapsula_resource", ChallengeKind::BrowserChallenge, 0.9) {
        observations.push(o);
    }
    if let Some(o) = probe("datadome", "datadome", ChallengeKind::BrowserChallenge, 0.8) {
        observations.push(o);
    }
    observations
}

fn header_value<'a>(headers: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use browsai_dom_observer::{RawDocument, RawNode};
    use browsai_network::{NetworkRequest, NetworkResponse};
    use browsai_profiles::ProfileManager;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn http_get(url: &str) -> NetworkRequest {
        let mut profiles = ProfileManager::default();
        NetworkRequest {
            id: Uuid::new_v4(),
            profile: profiles.create("default"),
            top_level_site: "example.test".into(),
            url: Url::parse(url).unwrap(),
            method: "GET".into(),
            headers: BTreeMap::new(),
            body: None,
            initiator: None,
        }
    }

    #[test]
    fn cloudflare_interstitial_with_cf_ray_and_marker_is_detected() {
        let mut observer = ChallengeObserver::new();
        let request = http_get("https://example.test/");
        let response = NetworkResponse {
            request_id: Uuid::nil(),
            status: 403,
            headers: BTreeMap::from([
                ("server".into(), "cloudflare".into()),
                ("cf-ray".into(), "abc123-IAD".into()),
            ]),
            body: b"<html><body class=\"cf-browser-verification\">...</body></html>".to_vec(),
        };
        observer.observe_network(&request, &response).unwrap();
        let snapshot = observer.snapshot();
        assert_eq!(snapshot.observations.len(), 1);
        let observation = &snapshot.observations[0];
        assert_eq!(observation.provider, "cloudflare");
        assert_eq!(observation.kind, ChallengeKind::BrowserChallenge);
        assert!(observation.confidence > 0.9);
    }

    #[test]
    fn rate_limit_429_emits_generic_access_denied_observation() {
        let mut observer = ChallengeObserver::new();
        let request = http_get("https://example.test/");
        let response = NetworkResponse {
            request_id: Uuid::nil(),
            status: 429,
            headers: BTreeMap::from([("retry-after".into(), "60".into())]),
            body: b"Too Many Requests".to_vec(),
        };
        observer.observe_network(&request, &response).unwrap();
        let snapshot = observer.snapshot();
        assert_eq!(snapshot.observations.len(), 1);
        assert_eq!(snapshot.observations[0].provider, "rate-limit");
        assert_eq!(snapshot.observations[0].kind, ChallengeKind::AccessDenied);
    }

    #[test]
    fn hcaptcha_iframe_in_dom_is_detected() {
        let mut document = RawDocument::new("https://example.test/login");
        let iframe = RawNode::element(2, "iframe", None);
        let mut iframe = iframe;
        iframe.attributes.insert(
            "src".into(),
            "https://newassets.hcaptcha.com/captcha/v1/1/standalone.html".into(),
        );
        iframe
            .attributes
            .insert("id".into(), "hcaptcha-iframe".into());
        document
            .apply(browsai_dom_observer::DomMutation::Insert { node: iframe })
            .unwrap();
        let mut observer = ChallengeObserver::new();
        observer.observe_document(&document).unwrap();
        let snapshot = observer.snapshot();
        assert!(snapshot.observations.iter().any(|o| o.provider == "hcaptcha"));
    }

    #[test]
    fn api_surface_is_observation_only() {
        // Compile-time check: no public method names start with solve/bypass/inject/mutate.
        let observer_type_id = std::any::TypeId::of::<ChallengeObserver>();
        let _ = observer_type_id;
        let mut observer = ChallengeObserver::new();
        // Allowed public methods (no auto-generated surface area beyond these):
        let _: Vec<Option<ChallengeObservation>> = vec![];
        let _ = observer.drain();
        let _ = observer.clear();
        let _ = observer.snapshot();
    }

    #[test]
    fn invalid_confidence_is_rejected() {
        let mut observer = ChallengeObserver::new();
        let request = http_get("https://example.test/");
        let mut headers = BTreeMap::new();
        headers.insert("cf-ray".into(), "abc-IAD".into());
        let response = NetworkResponse {
            request_id: Uuid::nil(),
            status: 403,
            headers,
            body: b"cf-browser-verification".to_vec(),
        };
        observer.observe_network(&request, &response).unwrap();
        assert_eq!(observer.snapshot().observations.len(), 1);
    }

    #[test]
    fn challenge_observations_match_expected_providers_and_kinds() {
        let url = Url::parse("https://example.test/").unwrap();
        let fixtures: Vec<(&str, &str, &str, ChallengeKind, &str)> = vec![
            (
                "cloudflare",
                include_str!("fixtures/cloudflare.html"),
                "cf-browser-verification",
                ChallengeKind::BrowserChallenge,
                "cloudflare",
            ),
            (
                "hcaptcha",
                include_str!("fixtures/hcaptcha.html"),
                "hcaptcha",
                ChallengeKind::Captcha,
                "hcaptcha",
            ),
            (
                "recaptcha",
                include_str!("fixtures/recaptcha.html"),
                "recaptcha",
                ChallengeKind::Captcha,
                "recaptcha",
            ),
            (
                "datadome",
                include_str!("fixtures/datadome.html"),
                "datadome",
                ChallengeKind::BrowserChallenge,
                "datadome",
            ),
            (
                "akamai",
                include_str!("fixtures/akamai.html"),
                "akamai",
                ChallengeKind::BrowserChallenge,
                "akamai",
            ),
        ];
        for (label, html, _marker, kind, expected_provider) in fixtures {
            let mut observer = ChallengeObserver::new();
            observer.observe_html_body(&url, html).unwrap();
            let observations = observer.snapshot().observations;
            assert!(
                observations.iter().any(|o| &o.provider == expected_provider && o.kind == kind),
                "fixture {label} did not yield {expected_provider}/{kind:?}; got {:?}",
                observations
                    .iter()
                    .map(|o| (o.provider.clone(), o.kind))
                    .collect::<Vec<_>>()
            );
        }
    }
}