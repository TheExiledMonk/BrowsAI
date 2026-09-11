//! Conservative API discovery from observed network behavior.

use browsai_network::{NetworkRequest, NetworkResponse};
use browsai_provenance::{Confidence, ProvenanceSource, SourceKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub method: String,
    pub url: Url,
    pub status_codes: BTreeSet<u16>,
    pub response_fields: BTreeMap<String, String>,
    pub confidence: Confidence,
    pub origin: FactOrigin,
    pub schema_origin: FactOrigin,
    pub provenance: Vec<ProvenanceSource>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FactOrigin {
    Observed,
    Inferred,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetworkTiming {
    pub started_at_tick: u64,
    pub completed_at_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CausalEvidence {
    pub request_id: Uuid,
    pub entity_id: Option<String>,
    pub action_id: Option<String>,
    pub state_change: Option<String>,
    #[serde(default)]
    pub form_id: Option<String>,
    #[serde(default)]
    pub script_id: Option<String>,
    pub timing: NetworkTiming,
    pub provenance: Vec<ProvenanceSource>,
}

#[derive(Clone, Debug, Default)]
pub struct ApiDiscovery {
    endpoints: BTreeMap<(String, String), ApiEndpoint>,
    correlations: Vec<CausalEvidence>,
}

impl ApiDiscovery {
    pub fn observe(
        &mut self,
        request: &NetworkRequest,
        response: &NetworkResponse,
    ) -> &ApiEndpoint {
        let key = (
            request.method.to_ascii_uppercase(),
            request.url.as_str().into(),
        );
        let endpoint = self.endpoints.entry(key).or_insert_with(|| ApiEndpoint {
            method: request.method.to_ascii_uppercase(),
            url: request.url.clone(),
            status_codes: BTreeSet::new(),
            response_fields: BTreeMap::new(),
            confidence: Confidence::PROBABLE,
            origin: FactOrigin::Observed,
            schema_origin: FactOrigin::Observed,
            provenance: vec![ProvenanceSource {
                kind: SourceKind::NetworkResponse,
                reference: response.request_id.to_string(),
                detail: Some("observed response".into()),
            }],
        });
        endpoint.status_codes.insert(response.status);
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&response.body) {
            if let Some(object) = value.as_object() {
                for (field, value) in object {
                    endpoint
                        .response_fields
                        .insert(field.clone(), json_type(value).into());
                }
                endpoint.confidence = Confidence::STRONG;
                endpoint.schema_origin = FactOrigin::Inferred;
            }
        }
        endpoint
    }

    pub fn endpoint(&self, method: &str, url: &Url) -> Option<&ApiEndpoint> {
        self.endpoints
            .get(&(method.to_ascii_uppercase(), url.as_str().into()))
    }
    pub fn endpoints(&self) -> impl Iterator<Item = &ApiEndpoint> {
        self.endpoints.values()
    }
    pub fn correlate(&mut self, evidence: CausalEvidence) {
        self.correlations.push(evidence);
    }
    pub fn correlations(&self) -> &[CausalEvidence] {
        &self.correlations
    }
    pub fn correlate_request(
        &mut self,
        request: &NetworkRequest,
        started_at_tick: u64,
        completed_at_tick: u64,
        entity_id: Option<String>,
        action_id: Option<String>,
        state_change: Option<String>,
    ) {
        self.correlate(CausalEvidence {
            request_id: request.id,
            entity_id,
            action_id,
            state_change,
            form_id: None,
            script_id: None,
            timing: NetworkTiming {
                started_at_tick,
                completed_at_tick,
            },
            provenance: vec![ProvenanceSource {
                kind: SourceKind::NetworkRequest,
                reference: request.id.to_string(),
                detail: request.initiator.clone(),
            }],
        });
    }

    pub fn infer_form_endpoint(
        &mut self,
        method: &str,
        url: Url,
        form_id: impl Into<String>,
    ) -> &ApiEndpoint {
        self.infer_endpoint(method, url, SourceKind::EventHandler, form_id.into())
    }

    pub fn infer_runtime_endpoint(
        &mut self,
        method: &str,
        url: Url,
        script_id: impl Into<String>,
    ) -> &ApiEndpoint {
        self.infer_endpoint(method, url, SourceKind::JavaScript, script_id.into())
    }

    pub fn infer_application_endpoint(
        &mut self,
        method: &str,
        url: Url,
        entity_id: impl Into<String>,
    ) -> &ApiEndpoint {
        self.infer_endpoint(method, url, SourceKind::AgentInference, entity_id.into())
    }

    fn infer_endpoint(
        &mut self,
        method: &str,
        url: Url,
        kind: SourceKind,
        reference: String,
    ) -> &ApiEndpoint {
        let key = (method.to_ascii_uppercase(), url.as_str().to_owned());
        self.endpoints.entry(key).or_insert_with(|| ApiEndpoint {
            method: method.to_ascii_uppercase(),
            url,
            status_codes: BTreeSet::new(),
            response_fields: BTreeMap::new(),
            confidence: Confidence::PROBABLE,
            origin: FactOrigin::Inferred,
            schema_origin: FactOrigin::Inferred,
            provenance: vec![ProvenanceSource {
                kind,
                reference,
                detail: Some("inferred endpoint".into()),
            }],
        })
    }
}

fn json_type(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use browsai_profiles::ProfileManager;
    use uuid::Uuid;

    #[test]
    fn observed_json_schema_is_provenance_bound() {
        let mut profiles = ProfileManager::default();
        let profile = profiles.create("work");
        let url = Url::parse("https://example.test/api/orders").unwrap();
        let request = NetworkRequest {
            id: Uuid::new_v4(),
            profile,
            top_level_site: "https://example.test".into(),
            url: url.clone(),
            method: "get".into(),
            headers: BTreeMap::new(),
            body: None,
            initiator: Some("form".into()),
        };
        let response = NetworkResponse {
            request_id: request.id,
            status: 200,
            headers: BTreeMap::new(),
            body: br#"{"id":1,"name":"x"}"#.to_vec(),
        };
        let mut discovery = ApiDiscovery::default();
        let endpoint = discovery.observe(&request, &response);
        assert_eq!(
            endpoint.response_fields.get("id").map(String::as_str),
            Some("number")
        );
        assert_eq!(endpoint.provenance[0].kind, SourceKind::NetworkResponse);
        assert_eq!(endpoint.origin, FactOrigin::Observed);
        assert_eq!(endpoint.schema_origin, FactOrigin::Inferred);
        discovery.correlate_request(
            &request,
            10,
            15,
            Some("order:1".into()),
            Some("action:submit".into()),
            Some("order-created".into()),
        );
        assert_eq!(discovery.correlations()[0].timing.completed_at_tick, 15);
        assert_eq!(
            discovery.correlations()[0].entity_id.as_deref(),
            Some("order:1")
        );
        assert_eq!(
            discovery
                .infer_form_endpoint("post", url.clone(), "form:order")
                .provenance[0]
                .kind,
            SourceKind::EventHandler
        );
        assert_eq!(
            discovery
                .infer_runtime_endpoint(
                    "get",
                    Url::parse("https://example.test/api/runtime-orders").unwrap(),
                    "script:orders",
                )
                .provenance[0]
                .kind,
            SourceKind::JavaScript
        );
        assert_eq!(
            discovery
                .infer_application_endpoint(
                    "get",
                    Url::parse("https://example.test/api/application-orders").unwrap(),
                    "order:1",
                )
                .provenance[0]
                .kind,
            SourceKind::AgentInference
        );
    }
}
