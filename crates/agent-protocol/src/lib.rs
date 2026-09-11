//! Engine-independent query and explanation API for agent clients.

use browsai_agent_tree::{
    ActionDescriptor, AgentNode, AgentNodeId, AgentRenderTree, AgentValue, ApplicationType,
    Geometry, SemanticRole, StructuralRole,
};
use browsai_provenance::{Confidence, ProvenanceSource};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRequest {
    pub protocol_version: u16,
    pub request_id: Uuid,
    pub agent_id: String,
    pub session_id: String,
    pub operation: AgentOperation,
}

impl AgentRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                actual: self.protocol_version,
            });
        }
        if self.agent_id.trim().is_empty() || self.session_id.trim().is_empty() {
            return Err(ProtocolError::InvalidRequest);
        }
        match &self.operation {
            AgentOperation::Navigate { url } => {
                if url.trim().is_empty() {
                    Err(ProtocolError::InvalidRequest)
                } else {
                    url::Url::parse(url)
                        .map(|_| ())
                        .map_err(|_| ProtocolError::InvalidUrl)
                }
            }
            AgentOperation::Action { target, action, .. }
                if target.trim().is_empty() || action.trim().is_empty() =>
            {
                Err(ProtocolError::InvalidRequest)
            }
            AgentOperation::Subscribe { stream } if stream.trim().is_empty() => {
                Err(ProtocolError::InvalidRequest)
            }
            AgentOperation::AcquireLease { ttl_ticks, .. } if *ttl_ticks == 0 => {
                Err(ProtocolError::InvalidRequest)
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AgentOperation {
    #[serde(rename = "query")]
    Query(Query),
    #[serde(rename = "navigate")]
    Navigate { url: String },
    #[serde(rename = "action")]
    Action {
        target: String,
        action: String,
        parameters: serde_json::Value,
    },
    #[serde(rename = "subscribe")]
    Subscribe { stream: String },
    #[serde(rename = "acquireLease")]
    AcquireLease { scope: LeaseScope, ttl_ticks: u64 },
    #[serde(rename = "releaseLease")]
    ReleaseLease { lease_id: LeaseId },
    #[serde(rename = "interrupt")]
    Interrupt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResponse {
    pub protocol_version: u16,
    pub request_id: Uuid,
    pub result: Result<AgentResult, ProtocolError>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AgentResult {
    Accepted,
    Query(Vec<QueryResult>),
    Lease(Lease),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProtocolError {
    VersionMismatch { expected: u16, actual: u16 },
    InvalidSession,
    LeaseHeld,
    LeaseNotFound,
    LeaseOwnerMismatch,
    Interrupted,
    InvalidRequest,
    InvalidUrl,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum LeaseScope {
    Page(String),
    Workspace(String),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct LeaseId(Uuid);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Lease {
    pub id: LeaseId,
    pub owner: String,
    pub scope: LeaseScope,
    pub expires_at_tick: u64,
}

#[derive(Clone, Debug, Default)]
pub struct LeaseManager {
    leases: BTreeMap<LeaseId, Lease>,
    interrupted: BTreeMap<String, u64>,
}

impl LeaseManager {
    pub fn acquire(
        &mut self,
        owner: impl Into<String>,
        scope: LeaseScope,
        ttl_ticks: u64,
        now: u64,
    ) -> Result<Lease, ProtocolError> {
        self.expire(now);
        if self.leases.values().any(|lease| lease.scope == scope) {
            return Err(ProtocolError::LeaseHeld);
        }
        let lease = Lease {
            id: LeaseId(Uuid::new_v4()),
            owner: owner.into(),
            scope,
            expires_at_tick: now.saturating_add(ttl_ticks.max(1)),
        };
        self.leases.insert(lease.id, lease.clone());
        Ok(lease)
    }

    pub fn release(&mut self, lease_id: LeaseId, owner: &str) -> Result<(), ProtocolError> {
        let lease = self
            .leases
            .get(&lease_id)
            .ok_or(ProtocolError::LeaseNotFound)?;
        if lease.owner != owner {
            return Err(ProtocolError::LeaseOwnerMismatch);
        }
        self.leases.remove(&lease_id);
        Ok(())
    }

    pub fn interrupt(&mut self, owner: impl Into<String>, now: u64) {
        self.interrupted.insert(owner.into(), now);
    }

    pub fn is_interrupted(&self, owner: &str) -> bool {
        self.interrupted.contains_key(owner)
    }
    pub fn active(&mut self, now: u64) -> impl Iterator<Item = &Lease> {
        self.expire(now);
        self.leases.values()
    }

    fn expire(&mut self, now: u64) {
        self.leases.retain(|_, lease| lease.expires_at_tick > now);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub role: Option<StructuralRole>,
    pub semantic_role: Option<SemanticRole>,
    pub application_type: Option<ApplicationType>,
    pub name: Option<String>,
    pub name_contains: Option<String>,
    pub identity_key: Option<String>,
    pub origin: Option<String>,
    pub visible: Option<bool>,
    pub enabled: Option<bool>,
    pub focused: Option<bool>,
    pub selected: Option<bool>,
    pub geometry: Option<GeometryConstraint>,
    pub relationship_kind: Option<String>,
    pub relationship_target: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryConstraint {
    pub min_x: Option<f64>,
    pub max_x: Option<f64>,
    pub min_y: Option<f64>,
    pub max_y: Option<f64>,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
}

impl Query {
    pub fn role(role: StructuralRole) -> Self {
        Self {
            role: Some(role),
            ..Default::default()
        }
    }
    fn matches(&self, node: &AgentNode) -> bool {
        self.role
            .as_ref()
            .map_or(true, |role| role == &node.structural_role)
            && self
                .semantic_role
                .as_ref()
                .map_or(true, |role| node.semantic_role.as_ref() == Some(role))
            && self
                .application_type
                .as_ref()
                .map_or(true, |kind| node.application_type.as_ref() == Some(kind))
            && self.name.as_ref().map_or(true, |name| {
                node.name
                    .as_ref()
                    .is_some_and(|actual| actual.eq_ignore_ascii_case(name))
            })
            && self.name_contains.as_ref().map_or(true, |needle| {
                node.name.as_ref().is_some_and(|actual| {
                    actual
                        .to_ascii_lowercase()
                        .contains(&needle.to_ascii_lowercase())
                })
            })
            && self
                .identity_key
                .as_ref()
                .map_or(true, |key| node.identity_key.as_ref() == Some(key))
            && self
                .origin
                .as_ref()
                .map_or(true, |origin| node.origin.as_ref() == Some(origin))
            && self
                .visible
                .map_or(true, |visible| node.state.visible == visible)
            && self
                .enabled
                .map_or(true, |enabled| node.state.enabled == enabled)
            && self
                .focused
                .map_or(true, |focused| node.state.focused == focused)
            && self
                .selected
                .map_or(true, |selected| node.state.selected == selected)
            && self.geometry.as_ref().map_or(true, |constraint| {
                node.geometry.as_ref().is_some_and(|geometry| {
                    constraint.min_x.map_or(true, |value| geometry.x >= value)
                        && constraint.max_x.map_or(true, |value| geometry.x <= value)
                        && constraint.min_y.map_or(true, |value| geometry.y >= value)
                        && constraint.max_y.map_or(true, |value| geometry.y <= value)
                        && constraint
                            .min_width
                            .map_or(true, |value| geometry.width >= value)
                        && constraint
                            .min_height
                            .map_or(true, |value| geometry.height >= value)
                })
            })
            && self.relationship_kind.as_ref().map_or(true, |kind| {
                node.relationships.iter().any(|relationship| {
                    relationship.kind == *kind
                        && self
                            .relationship_target
                            .as_ref()
                            .map_or(true, |target| relationship.target == *target)
                })
            })
            && self.relationship_target.as_ref().map_or(true, |target| {
                node.relationships
                    .iter()
                    .any(|relationship| relationship.target == *target)
            })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub node_id: AgentNodeId,
    pub name: Option<String>,
    pub role: StructuralRole,
    pub semantic_role: Option<SemanticRole>,
    pub value: Option<AgentValue>,
    pub confidence: Confidence,
    pub provenance: Vec<ProvenanceSource>,
    pub geometry: Option<Geometry>,
    pub origin: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryShape {
    pub max_results: Option<usize>,
    pub max_tokens: Option<usize>,
    pub include_value: bool,
    pub include_provenance: bool,
    pub include_geometry: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryPage {
    pub results: Vec<QueryResult>,
    /// Cursor used for this page. Kept alongside `offset` for wire clarity.
    pub cursor: usize,
    pub limit: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
    pub truncated: bool,
    /// String cursor for clients that treat cursors as opaque tokens.
    pub next_cursor: Option<String>,
    pub total: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum QueryResolutionError {
    NotFound,
    Ambiguous { matches: usize },
    StaleReference { expected: u64, actual: u64 },
}

impl From<&AgentNode> for QueryResult {
    fn from(node: &AgentNode) -> Self {
        Self {
            node_id: node.id.clone(),
            name: node.name.clone(),
            role: node.structural_role.clone(),
            semantic_role: node.semantic_role.clone(),
            value: node.value.clone(),
            confidence: node.confidence,
            provenance: node.provenance.clone(),
            geometry: node.geometry.clone(),
            origin: node.origin.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Explanation {
    pub node_id: AgentNodeId,
    pub name: Option<String>,
    pub role: StructuralRole,
    pub semantic_role: Option<SemanticRole>,
    pub confidence: Confidence,
    pub provenance: Vec<ProvenanceSource>,
    pub actions: Vec<ActionDescriptor>,
}

pub struct PageQuery<'a> {
    tree: &'a AgentRenderTree,
}

impl<'a> PageQuery<'a> {
    pub fn new(tree: &'a AgentRenderTree) -> Self {
        Self { tree }
    }
    pub fn render(&self, max_nodes: Option<usize>) -> Vec<QueryResult> {
        self.tree
            .nodes
            .iter()
            .take(max_nodes.unwrap_or(usize::MAX))
            .map(QueryResult::from)
            .collect()
    }

    pub fn render_page(&self, offset: usize, limit: usize) -> QueryPage {
        let total = self.tree.nodes.len();
        let start = offset.min(total);
        let end = start.saturating_add(limit.max(1)).min(total);
        QueryPage {
            results: self.tree.nodes[start..end]
                .iter()
                .map(QueryResult::from)
                .collect(),
            cursor: start,
            limit: limit.max(1),
            offset: start,
            next_offset: (end < total).then_some(end),
            truncated: end < total,
            next_cursor: (end < total).then_some(end.to_string()),
            total,
        }
    }
    pub fn query(&self, query: &Query) -> Vec<QueryResult> {
        self.tree
            .nodes
            .iter()
            .filter(|node| query.matches(node))
            .map(QueryResult::from)
            .collect()
    }

    pub fn search(&self, text: &str) -> Vec<QueryResult> {
        let query = Query {
            name_contains: Some(text.to_owned()),
            ..Default::default()
        };
        self.query(&query)
    }

    pub fn query_page(&self, query: &Query, offset: usize, limit: usize) -> QueryPage {
        let matches: Vec<_> = self
            .tree
            .nodes
            .iter()
            .filter(|node| query.matches(node))
            .collect();
        let total = matches.len();
        let start = offset.min(total);
        let end = start.saturating_add(limit.max(1)).min(total);
        QueryPage {
            results: matches[start..end]
                .iter()
                .map(|node| QueryResult::from(*node))
                .collect(),
            cursor: start,
            limit: limit.max(1),
            offset: start,
            next_offset: (end < total).then_some(end),
            truncated: end < total,
            next_cursor: (end < total).then_some(end.to_string()),
            total,
        }
    }

    /// Applies deterministic result and approximate-token limits after query matching.
    /// Serialized JSON size is used as a conservative byte/token estimate.
    pub fn query_shaped(&self, query: &Query, shape: QueryShape) -> Vec<QueryResult> {
        let mut used = 0usize;
        let mut results = Vec::new();
        for node in self.tree.nodes.iter().filter(|node| query.matches(node)) {
            if shape
                .max_results
                .is_some_and(|limit| results.len() >= limit)
            {
                break;
            }
            let mut result = QueryResult::from(node);
            if !shape.include_value {
                result.value = None;
            }
            if !shape.include_provenance {
                result.provenance.clear();
            }
            if !shape.include_geometry {
                result.geometry = None;
            }
            let estimated_tokens = serde_json::to_string(&result)
                .map(|value| value.len().saturating_add(3) / 4)
                .unwrap_or(usize::MAX);
            if shape
                .max_tokens
                .is_some_and(|limit| !results.is_empty() && used + estimated_tokens > limit)
            {
                break;
            }
            used = used.saturating_add(estimated_tokens);
            results.push(result);
        }
        results
    }
    pub fn get(&self, node_id: &str) -> Option<QueryResult> {
        self.tree.find(node_id).map(QueryResult::from)
    }
    pub fn find(&self, query: &Query) -> Option<QueryResult> {
        self.tree
            .nodes
            .iter()
            .find(|node| query.matches(node))
            .map(QueryResult::from)
    }

    pub fn resolve(&self, query: &Query) -> Result<QueryResult, QueryResolutionError> {
        let matches = self
            .tree
            .nodes
            .iter()
            .filter(|node| query.matches(node))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(QueryResolutionError::NotFound),
            [node] => Ok(QueryResult::from(*node)),
            _ => Err(QueryResolutionError::Ambiguous {
                matches: matches.len(),
            }),
        }
    }

    pub fn resolve_at_generation(
        &self,
        query: &Query,
        expected_generation: u64,
    ) -> Result<QueryResult, QueryResolutionError> {
        if self.tree.generation != expected_generation {
            return Err(QueryResolutionError::StaleReference {
                expected: expected_generation,
                actual: self.tree.generation,
            });
        }
        self.resolve(query)
    }
    pub fn actions(&self, node_id: &str) -> Option<&[ActionDescriptor]> {
        self.tree.find(node_id).map(|node| node.actions.as_slice())
    }
    pub fn explain(&self, node_id: &str) -> Option<Explanation> {
        self.tree.find(node_id).map(|node| Explanation {
            node_id: node.id.clone(),
            name: node.name.clone(),
            role: node.structural_role.clone(),
            semantic_role: node.semantic_role.clone(),
            confidence: node.confidence,
            provenance: node.provenance.clone(),
            actions: node.actions.clone(),
        })
    }
}
