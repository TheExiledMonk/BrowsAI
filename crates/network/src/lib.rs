//! Typed network lifecycle and observation contracts.

use browsai_cache::{CachedResponse, HttpCache};
use browsai_cookies::CookieJar;
use browsai_engine_api::ProfileIdentity;
use browsai_profiles::ProfileId;
use browsai_service_worker::{FetchResponse, Registration, ServiceWorkerRegistry, WorkerState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NetworkRequest {
    pub id: Uuid,
    pub profile: ProfileId,
    pub top_level_site: String,
    pub url: Url,
    pub method: String,
    pub headers: BTreeMap<String, String>,
    pub body: Option<Vec<u8>>,
    pub initiator: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetworkResponse {
    pub request_id: Uuid,
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NetworkEvent {
    RequestStarted {
        request_id: Uuid,
        url: Url,
    },
    RequestInitiator {
        request_id: Uuid,
        initiator: String,
    },
    ResponseReceived {
        request_id: Uuid,
        status: u16,
        bytes: usize,
    },
    Timing {
        request_id: Uuid,
        elapsed_millis: u64,
    },
    RequestCompleted {
        request_id: Uuid,
        status: u16,
    },
    RequestFailed {
        request_id: Uuid,
        error: String,
    },
    Mutation {
        request_id: Uuid,
        kind: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkError {
    UnsupportedScheme,
    UntrustedCertificate,
    NoRoute,
    InvalidMethod,
    RequestLimit,
    ResponseTooLarge,
    Timeout,
    RedirectLimit,
    InvalidRedirect,
    WebSocketDisabled,
    WebSocketNoRoute,
    WorkersDisabled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NormalizedNetworkError {
    UnsupportedScheme,
    UntrustedCertificate,
    InvalidMethod,
    NoRoute,
    RequestLimit,
    ResponseTooLarge,
    Timeout,
    RedirectLimit,
    InvalidRedirect,
    WebSocketDisabled,
    WebSocketNoRoute,
    WorkersDisabled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NetworkAuditEvent {
    Completed {
        request_id: Uuid,
        status: u16,
        bytes: usize,
    },
    Failed {
        request_id: Uuid,
        error: NormalizedNetworkError,
    },
    CookieStored {
        request_id: Uuid,
        domain: String,
    },
}

impl From<&NetworkError> for NormalizedNetworkError {
    fn from(error: &NetworkError) -> Self {
        match error {
            NetworkError::UnsupportedScheme => Self::UnsupportedScheme,
            NetworkError::UntrustedCertificate => Self::UntrustedCertificate,
            NetworkError::InvalidMethod => Self::InvalidMethod,
            NetworkError::NoRoute => Self::NoRoute,
            NetworkError::RequestLimit => Self::RequestLimit,
            NetworkError::ResponseTooLarge => Self::ResponseTooLarge,
            NetworkError::Timeout => Self::Timeout,
            NetworkError::RedirectLimit => Self::RedirectLimit,
            NetworkError::InvalidRedirect => Self::InvalidRedirect,
            NetworkError::WebSocketDisabled => Self::WebSocketDisabled,
            NetworkError::WebSocketNoRoute => Self::WebSocketNoRoute,
            NetworkError::WorkersDisabled => Self::WorkersDisabled,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebSocketFrame {
    Text(String),
    Binary(Vec<u8>),
    Close,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSocketConnection {
    pub url: Url,
    incoming: Vec<WebSocketFrame>,
    sent: Vec<WebSocketFrame>,
    closed: bool,
}

impl WebSocketConnection {
    pub fn send(&mut self, frame: WebSocketFrame) -> Result<(), NetworkError> {
        if self.closed {
            return Err(NetworkError::WebSocketNoRoute);
        }
        if frame == WebSocketFrame::Close {
            self.closed = true;
        }
        self.sent.push(frame);
        Ok(())
    }

    pub fn receive(&mut self) -> Option<WebSocketFrame> {
        if self.incoming.is_empty() {
            None
        } else {
            Some(self.incoming.remove(0))
        }
    }

    pub fn sent_frames(&self) -> &[WebSocketFrame] {
        &self.sent
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetworkLimits {
    pub max_requests: u64,
    pub max_response_bytes: usize,
    pub timeout_millis: u64,
    pub max_redirects: u8,
}

impl Default for NetworkLimits {
    fn default() -> Self {
        Self {
            max_requests: 10_000,
            max_response_bytes: 16 * 1024 * 1024,
            timeout_millis: 30_000,
            max_redirects: 10,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub proxy: Option<Url>,
    pub trusted_certificates: Vec<String>,
    pub websocket_enabled: bool,
    pub workers_enabled: bool,
    pub service_workers_enabled: bool,
    #[serde(default)]
    pub profile_identity: Option<ProfileIdentity>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub requests_started: u64,
    pub requests_completed: u64,
    pub requests_failed: u64,
    pub bytes_received: u64,
}

#[derive(Clone, Debug, Default)]
pub struct NetworkStack {
    routes: BTreeMap<(String, String), NetworkResponse>,
    websocket_routes: BTreeMap<String, Vec<WebSocketFrame>>,
    events: Vec<NetworkEvent>,
    pub cookies: CookieJar,
    pub cache: HttpCache,
    pub service_workers: ServiceWorkerRegistry,
    pub limits: NetworkLimits,
    pub metrics: NetworkMetrics,
    pub config: NetworkConfig,
    pub logical_time_millis: u64,
    audit_events: Vec<NetworkAuditEvent>,
}

impl NetworkStack {
    pub fn set_profile_identity(&mut self, identity: ProfileIdentity) {
        self.config.profile_identity = Some(identity);
    }

    pub fn apply_profile_headers(
        headers: &mut BTreeMap<String, String>,
        identity: Option<&ProfileIdentity>,
    ) {
        let Some(identity) = identity else {
            return;
        };
        let mut entries: Vec<(String, String)> = Vec::new();
        let has_header = |name: &str| headers.keys().any(|key| key.eq_ignore_ascii_case(name));
        if !has_header("user-agent") {
            entries.push(("user-agent".into(), identity.user_agent.clone()));
        }
        if !has_header("accept-language") {
            entries.push(("accept-language".into(), identity.accept_language.clone()));
        }
        if !has_header("sec-ch-ua") && !identity.brands.is_empty() {
            entries.push(("sec-ch-ua".into(), identity.sec_ch_ua()));
        }
        if !has_header("sec-ch-ua-mobile") {
            entries.push((
                "sec-ch-ua-mobile".into(),
                identity.sec_ch_ua_mobile().into(),
            ));
        }
        if !has_header("sec-ch-ua-platform") {
            entries.push(("sec-ch-ua-platform".into(), identity.sec_ch_ua_platform()));
        }
        for (name, value) in entries {
            headers.insert(name, value);
        }
    }

    fn correlate_response(request_id: Uuid, mut response: NetworkResponse) -> NetworkResponse {
        response.request_id = request_id;
        response
    }

    pub fn route(&mut self, method: impl Into<String>, url: &Url, response: NetworkResponse) {
        self.routes.insert(
            (method.into().to_ascii_uppercase(), url.as_str().to_owned()),
            response,
        );
    }

    pub fn route_websocket(&mut self, url: &Url, incoming: Vec<WebSocketFrame>) {
        self.websocket_routes
            .insert(url.as_str().to_owned(), incoming);
    }

    pub fn register_service_worker(&mut self, scope: Url, script_url: Url) -> Registration {
        self.service_workers.register(scope, script_url)
    }

    pub fn transition_service_worker(
        &mut self,
        scope: &Url,
        next: WorkerState,
    ) -> Option<Registration> {
        self.service_workers.transition(scope, next)
    }

    pub fn unregister_service_worker(&mut self, scope: &Url) -> bool {
        self.service_workers.unregister(scope)
    }

    pub fn intercept_service_worker(&mut self, url: &Url, response: FetchResponse) {
        self.service_workers.intercept(url, response);
    }

    pub fn advance_time(&mut self, millis: u64) {
        self.logical_time_millis = self.logical_time_millis.saturating_add(millis);
    }

    pub fn invalidate_cache(&mut self, profile: &ProfileId, partition: &str, url: &Url) {
        self.cache.invalidate(profile, partition, url);
    }

    pub fn invalidate_cache_partition(&mut self, profile: &ProfileId, partition: &str) {
        self.cache.invalidate_partition(profile, partition);
    }

    pub fn export_cookie_snapshot(&self) -> Result<String, serde_json::Error> {
        self.cookies.to_json()
    }

    pub fn restore_cookie_snapshot(&mut self, value: &str) -> Result<(), serde_json::Error> {
        self.cookies = CookieJar::from_json(value)?;
        Ok(())
    }

    pub fn connect_websocket(&self, url: &Url) -> Result<WebSocketConnection, NetworkError> {
        if !self.config.websocket_enabled {
            return Err(NetworkError::WebSocketDisabled);
        }
        if !matches!(url.scheme(), "ws" | "wss") {
            return Err(NetworkError::UnsupportedScheme);
        }
        let incoming = self
            .websocket_routes
            .get(url.as_str())
            .cloned()
            .ok_or(NetworkError::WebSocketNoRoute)?;
        Ok(WebSocketConnection {
            url: url.clone(),
            incoming,
            sent: Vec::new(),
            closed: false,
        })
    }

    pub fn fetch(&mut self, mut request: NetworkRequest) -> Result<NetworkResponse, NetworkError> {
        if self.metrics.requests_started >= self.limits.max_requests {
            self.audit_failure(request.id, &NetworkError::RequestLimit);
            return Err(NetworkError::RequestLimit);
        }
        if !matches!(request.url.scheme(), "http" | "https") {
            self.audit_failure(request.id, &NetworkError::UnsupportedScheme);
            return Err(NetworkError::UnsupportedScheme);
        }
        if request.url.scheme() == "https"
            && !self.config.trusted_certificates.is_empty()
            && request.url.host_str().map_or(true, |host| {
                !self
                    .config
                    .trusted_certificates
                    .iter()
                    .any(|trusted| trusted.eq_ignore_ascii_case(host))
            })
        {
            self.audit_failure(request.id, &NetworkError::UntrustedCertificate);
            return Err(NetworkError::UntrustedCertificate);
        }
        if request.method.is_empty() {
            self.audit_failure(request.id, &NetworkError::InvalidMethod);
            return Err(NetworkError::InvalidMethod);
        }
        request.method = request.method.to_ascii_uppercase();
        let started_at = self.logical_time_millis;
        let cookie_header =
            self.cookies
                .header_for(&request.profile, &request.top_level_site, &request.url);
        if !cookie_header.is_empty() {
            request.headers.insert("cookie".into(), cookie_header);
        }
        Self::apply_profile_headers(&mut request.headers, self.config.profile_identity.as_ref());
        self.events.push(NetworkEvent::RequestStarted {
            request_id: request.id,
            url: request.url.clone(),
        });
        if let Some(initiator) = request.initiator.clone() {
            self.events.push(NetworkEvent::RequestInitiator {
                request_id: request.id,
                initiator,
            });
        }
        self.metrics.requests_started += 1;
        if self.limits.timeout_millis == 0 {
            self.metrics.requests_failed += 1;
            self.events.push(NetworkEvent::RequestFailed {
                request_id: request.id,
                error: format!("{:?}", NetworkError::Timeout),
            });
            self.audit_failure(request.id, &NetworkError::Timeout);
            return Err(NetworkError::Timeout);
        }
        if self.config.service_workers_enabled {
            if let Some(intercepted) = self.service_workers.fetch(&request.url).cloned() {
                let response = Self::correlate_response(
                    request.id,
                    NetworkResponse {
                        request_id: request.id,
                        status: intercepted.status,
                        headers: BTreeMap::new(),
                        body: intercepted.body,
                    },
                );
                self.metrics.requests_completed += 1;
                self.metrics.bytes_received += response.body.len() as u64;
                self.events.push(NetworkEvent::ResponseReceived {
                    request_id: request.id,
                    status: response.status,
                    bytes: response.body.len(),
                });
                self.events.push(NetworkEvent::Timing {
                    request_id: request.id,
                    elapsed_millis: self.logical_time_millis.saturating_sub(started_at),
                });
                self.events.push(NetworkEvent::RequestCompleted {
                    request_id: request.id,
                    status: response.status,
                });
                self.audit_completed(request.id, response.status, response.body.len());
                return Ok(response);
            }
        }
        if request.method == "GET" {
            if let Some(cached) = self
                .cache
                .get(&request.profile, &request.top_level_site, &request.url)
                .cloned()
            {
                let response = Self::correlate_response(
                    request.id,
                    NetworkResponse {
                        request_id: request.id,
                        status: cached.status,
                        headers: cached.headers,
                        body: cached.body,
                    },
                );
                self.metrics.requests_completed += 1;
                self.metrics.bytes_received += response.body.len() as u64;
                self.events.push(NetworkEvent::ResponseReceived {
                    request_id: request.id,
                    status: response.status,
                    bytes: response.body.len(),
                });
                self.events.push(NetworkEvent::Timing {
                    request_id: request.id,
                    elapsed_millis: self.logical_time_millis.saturating_sub(started_at),
                });
                self.events.push(NetworkEvent::RequestCompleted {
                    request_id: request.id,
                    status: response.status,
                });
                self.audit_completed(request.id, response.status, response.body.len());
                return Ok(response);
            }
        }
        let mut redirects = 0;
        let response = loop {
            let direct_key = (request.method.clone(), request.url.as_str().to_owned());
            let proxy_key = self
                .config
                .proxy
                .as_ref()
                .map(|proxy| (request.method.clone(), proxy.as_str().to_owned()));
            let response = proxy_key
                .as_ref()
                .and_then(|key| self.routes.get(key))
                .or_else(|| self.routes.get(&direct_key))
                .cloned();
            let Some(response) = response else {
                break Err(NetworkError::NoRoute);
            };
            if (300..400).contains(&response.status) {
                let Some(location) = response.headers.get("location") else {
                    break Ok(response);
                };
                if redirects >= self.limits.max_redirects {
                    break Err(NetworkError::RedirectLimit);
                }
                let Ok(next_url) = request.url.join(location) else {
                    break Err(NetworkError::InvalidRedirect);
                };
                request.url = next_url;
                redirects += 1;
                continue;
            }
            break Ok(response);
        };
        let response = response.map(|response| Self::correlate_response(request.id, response));
        match &response {
            Ok(response) if response.body.len() > self.limits.max_response_bytes => {
                self.metrics.requests_failed += 1;
                self.events.push(NetworkEvent::RequestFailed {
                    request_id: request.id,
                    error: format!("{:?}", NetworkError::ResponseTooLarge),
                });
                self.audit_failure(request.id, &NetworkError::ResponseTooLarge);
                return Err(NetworkError::ResponseTooLarge);
            }
            Ok(response) => {
                if let Some((_, set_cookie)) = response
                    .headers
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
                {
                    if let Some(cookie) = browsai_cookies::Cookie::parse(set_cookie, &request.url) {
                        let domain = cookie.domain.clone();
                        self.cookies
                            .set(&request.profile, &request.top_level_site, cookie);
                        self.audit_events.push(NetworkAuditEvent::CookieStored {
                            request_id: request.id,
                            domain,
                        });
                    }
                }
                if request.method == "GET" && (200..400).contains(&response.status) {
                    self.cache.put(
                        &request.profile,
                        &request.top_level_site,
                        &request.url,
                        CachedResponse {
                            status: response.status,
                            headers: response.headers.clone(),
                            body: response.body.clone(),
                        },
                    );
                } else if request.method != "GET" {
                    self.cache
                        .invalidate(&request.profile, &request.top_level_site, &request.url);
                    self.events.push(NetworkEvent::Mutation {
                        request_id: request.id,
                        kind: "cacheInvalidated".into(),
                    });
                }
                self.metrics.requests_completed += 1;
                self.metrics.bytes_received += response.body.len() as u64;
                self.events.push(NetworkEvent::ResponseReceived {
                    request_id: request.id,
                    status: response.status,
                    bytes: response.body.len(),
                });
                self.events.push(NetworkEvent::Timing {
                    request_id: request.id,
                    elapsed_millis: self.logical_time_millis.saturating_sub(started_at),
                });
                self.events.push(NetworkEvent::RequestCompleted {
                    request_id: request.id,
                    status: response.status,
                });
                self.audit_completed(request.id, response.status, response.body.len());
            }
            Err(error) => {
                self.metrics.requests_failed += 1;
                self.events.push(NetworkEvent::Timing {
                    request_id: request.id,
                    elapsed_millis: self.logical_time_millis.saturating_sub(started_at),
                });
                self.events.push(NetworkEvent::RequestFailed {
                    request_id: request.id,
                    error: format!("{error:?}"),
                });
                self.audit_failure(request.id, error);
            }
        }
        response
    }

    pub fn fetch_worker(
        &mut self,
        mut request: NetworkRequest,
    ) -> Result<NetworkResponse, NetworkError> {
        if !self.config.workers_enabled {
            self.audit_failure(request.id, &NetworkError::WorkersDisabled);
            return Err(NetworkError::WorkersDisabled);
        }
        request.initiator = Some("worker".into());
        self.fetch(request)
    }
    pub fn events(&self) -> &[NetworkEvent] {
        &self.events
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = NetworkEvent> + '_ {
        self.events.drain(..)
    }

    pub fn audit_events(&self) -> &[NetworkAuditEvent] {
        &self.audit_events
    }

    pub fn drain_audit_events(&mut self) -> impl Iterator<Item = NetworkAuditEvent> + '_ {
        self.audit_events.drain(..)
    }

    fn audit_completed(&mut self, request_id: Uuid, status: u16, bytes: usize) {
        self.audit_events.push(NetworkAuditEvent::Completed {
            request_id,
            status,
            bytes,
        });
    }

    fn audit_failure(&mut self, request_id: Uuid, error: &NetworkError) {
        self.audit_events.push(NetworkAuditEvent::Failed {
            request_id,
            error: NormalizedNetworkError::from(error),
        });
    }
}
