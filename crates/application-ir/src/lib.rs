use browsai_agent_tree::{AgentNode, AgentRenderTree, ApplicationType};
use browsai_api_discovery::CausalEvidence;
use browsai_provenance::{Confidence, ExplainabilityReport, ProvenanceSource};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplicationIr {
    pub tree: AgentRenderTree,
    pub graph: ApplicationGraph,
    pub causal_evidence: Vec<CausalEvidence>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplicationEntity {
    pub id: String,
    pub kind: ApplicationType,
    pub fields: std::collections::BTreeMap<String, String>,
    pub source_nodes: Vec<String>,
    pub confidence: Confidence,
    pub provenance: Vec<ProvenanceSource>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationQuery {
    pub id: Option<String>,
    pub kind: Option<ApplicationType>,
    pub collection_id: Option<String>,
}

/// A bounded page of application entities.
///
/// Collection membership can describe a virtualized or otherwise large
/// result set, so callers must be able to consume it incrementally instead of
/// materializing every matching entity in one response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplicationEntityPage {
    pub results: Vec<ApplicationEntity>,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
    pub truncated: bool,
    pub next_offset: Option<usize>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub action: String,
    pub target: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub score: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplicationTable {
    pub id: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplicationForm {
    pub id: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplicationDialog {
    pub id: String,
    pub title: String,
    pub modal: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplicationNotification {
    pub id: String,
    pub message: String,
    pub level: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplicationRelation {
    pub from: String,
    pub relation: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApplicationCollection {
    pub id: String,
    pub kind: Option<ApplicationType>,
    pub members: Vec<String>,
    pub total_known: Option<usize>,
    pub virtualized: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ApplicationGraph {
    pub entities: Vec<ApplicationEntity>,
    pub relations: Vec<ApplicationRelation>,
    pub collections: Vec<ApplicationCollection>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SiteLearningEntry {
    pub origin: String,
    pub observed_roles: Vec<String>,
    pub observations: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SiteLearningCache {
    max_entries: usize,
    entries: BTreeMap<String, SiteLearningEntry>,
}

impl Default for SiteLearningCache {
    fn default() -> Self {
        Self {
            max_entries: 128,
            entries: BTreeMap::new(),
        }
    }
}

impl SiteLearningCache {
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            max_entries: max_entries.max(1),
            ..Self::default()
        }
    }

    pub fn observe(&mut self, origin: impl Into<String>, tree: &AgentRenderTree) {
        let origin = origin.into();
        let entry = self
            .entries
            .entry(origin.clone())
            .or_insert_with(|| SiteLearningEntry {
                origin: origin.clone(),
                observed_roles: vec![],
                observations: 0,
            });
        entry.observations = entry.observations.saturating_add(1);
        for role in tree
            .nodes
            .iter()
            .map(|node| format!("{:?}", node.structural_role))
        {
            if !entry.observed_roles.contains(&role) {
                entry.observed_roles.push(role);
                entry.observed_roles.sort();
            }
        }
        while self.entries.len() > self.max_entries {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.observations)
                .map(|(origin, _)| origin.clone())
            else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    pub fn get(&self, origin: &str) -> Option<&SiteLearningEntry> {
        self.entries.get(origin)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemanticCache {
    max_entries: usize,
    order: VecDeque<String>,
    entries: BTreeMap<String, AgentRenderTree>,
}

impl Default for SemanticCache {
    fn default() -> Self {
        Self {
            max_entries: 128,
            order: VecDeque::new(),
            entries: BTreeMap::new(),
        }
    }
}

impl SemanticCache {
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            max_entries: max_entries.max(1),
            ..Self::default()
        }
    }

    pub fn insert(&mut self, key: impl Into<String>, tree: AgentRenderTree) {
        let key = key.into();
        if self.entries.contains_key(&key) {
            self.order.retain(|existing| existing != &key);
        }
        self.order.push_back(key.clone());
        self.entries.insert(key, tree);
        while self.order.len() > self.max_entries {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&AgentRenderTree> {
        let position = self.order.iter().position(|existing| existing == key)?;
        let key = self.order.remove(position)?;
        self.order.push_back(key.clone());
        self.entries.get(&key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl ApplicationGraph {
    pub fn add_relation(&mut self, relation: ApplicationRelation) {
        if !self.relations.contains(&relation) {
            self.relations.push(relation);
        }
    }

    pub fn add_collection(&mut self, collection: ApplicationCollection) {
        if let Some(existing) = self
            .collections
            .iter_mut()
            .find(|existing| existing.id == collection.id)
        {
            *existing = collection;
        } else {
            self.collections.push(collection);
        }
    }
}

impl ApplicationIr {
    pub fn from_semantic(tree: AgentRenderTree) -> Self {
        let entities = tree
            .nodes
            .iter()
            .filter_map(|node| {
                node.application_type.clone().map(|kind| ApplicationEntity {
                    id: node.identity_key.clone().unwrap_or_else(|| node.id.clone()),
                    kind,
                    fields: node
                        .name
                        .clone()
                        .map(|name| [("name".into(), name)].into_iter().collect())
                        .unwrap_or_default(),
                    source_nodes: vec![node.id.clone()],
                    confidence: node.confidence,
                    provenance: node.provenance.clone(),
                })
            })
            .collect();
        Self {
            tree,
            graph: ApplicationGraph {
                entities,
                ..Default::default()
            },
            causal_evidence: Vec::new(),
        }
    }

    pub fn from_semantic_with_evidence(
        tree: AgentRenderTree,
        causal_evidence: Vec<CausalEvidence>,
    ) -> Self {
        let mut ir = Self::from_semantic(tree);
        ir.causal_evidence = causal_evidence;
        ir
    }

    pub fn add_causal_evidence(&mut self, evidence: CausalEvidence) {
        self.causal_evidence.push(evidence);
    }

    pub fn explainability_report(&self, subject: impl Into<String>) -> ExplainabilityReport {
        let mut report = ExplainabilityReport::new(
            subject,
            format!(
                "{} causal application observations",
                self.causal_evidence.len()
            ),
        );
        for evidence in &self.causal_evidence {
            for source in &evidence.provenance {
                report.add(
                    source.clone(),
                    Confidence::PROBABLE,
                    evidence.timing.completed_at_tick,
                    Some(format!(
                        "request={} action={:?} entity={:?} state={:?} elapsed_ticks={}",
                        evidence.request_id,
                        evidence.action_id,
                        evidence.entity_id,
                        evidence.state_change,
                        evidence
                            .timing
                            .completed_at_tick
                            .saturating_sub(evidence.timing.started_at_tick)
                    )),
                    false,
                );
            }
        }
        report
    }

    pub fn add_relation(&mut self, relation: ApplicationRelation) {
        self.graph.add_relation(relation);
    }

    pub fn add_collection(&mut self, collection: ApplicationCollection) {
        self.graph.add_collection(collection);
    }
    pub fn typed_nodes(&self, kind: ApplicationType) -> impl Iterator<Item = &AgentNode> {
        self.tree
            .nodes
            .iter()
            .filter(move |node| node.application_type == Some(kind.clone()))
    }

    pub fn collection(&self, id: &str) -> Option<&ApplicationCollection> {
        self.graph
            .collections
            .iter()
            .find(|collection| collection.id == id)
    }

    pub fn query_entities(&self, query: &ApplicationQuery) -> Vec<&ApplicationEntity> {
        self.graph
            .entities
            .iter()
            .filter(|entity| {
                query.id.as_ref().map_or(true, |id| &entity.id == id)
                    && query
                        .kind
                        .as_ref()
                        .map_or(true, |kind| &entity.kind == kind)
                    && query.collection_id.as_ref().map_or(true, |collection_id| {
                        self.collection(collection_id)
                            .is_some_and(|collection| collection.members.contains(&entity.id))
                    })
            })
            .collect()
    }

    /// Returns a bounded page without changing the legacy unbounded query API.
    pub fn query_entities_page(
        &self,
        query: &ApplicationQuery,
        offset: usize,
        limit: usize,
    ) -> ApplicationEntityPage {
        let matches: Vec<_> = self
            .graph
            .entities
            .iter()
            .filter(|entity| {
                query.id.as_ref().map_or(true, |id| &entity.id == id)
                    && query
                        .kind
                        .as_ref()
                        .map_or(true, |kind| &entity.kind == kind)
                    && query.collection_id.as_ref().map_or(true, |collection_id| {
                        self.collection(collection_id)
                            .is_some_and(|collection| collection.members.contains(&entity.id))
                    })
            })
            .collect();
        let total = matches.len();
        let start = offset.min(total);
        let effective_limit = limit.max(1);
        let end = start.saturating_add(effective_limit).min(total);
        let truncated = end < total;
        ApplicationEntityPage {
            results: matches[start..end]
                .iter()
                .map(|entity| (*entity).clone())
                .collect(),
            offset: start,
            limit: effective_limit,
            total,
            truncated,
            next_offset: truncated.then_some(end),
            next_cursor: truncated.then_some(end.to_string()),
        }
    }
}
