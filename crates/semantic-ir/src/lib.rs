use browsai_agent_tree::{AgentNode, AgentRenderTree, SemanticRole, StructuralRole};
use browsai_challenge_observer::{ChallengeKind, ChallengeObservationSet};
use browsai_fingerprint::FingerprintInconsistency;
use browsai_provenance::SourceKind;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemanticIr {
    pub tree: AgentRenderTree,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SemanticIssue {
    AmbiguousIdentity {
        identity_key: String,
        node_ids: Vec<String>,
    },
    MissingAccessibleName {
        node_id: String,
        structural_role: StructuralRole,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticNormalization {
    pub changed_nodes: Vec<String>,
    pub issues: Vec<SemanticIssue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemanticGoldenFixture {
    pub id: String,
    pub roles: Vec<StructuralRole>,
    pub labels: Vec<String>,
    pub relationship_kinds: Vec<String>,
    pub visible: bool,
    pub enabled: bool,
    pub geometry_count: usize,
    pub confidence: f32,
    pub provenance: Vec<SourceKind>,
}

impl SemanticIr {
    pub fn from_structural(tree: AgentRenderTree) -> Self {
        Self { tree }
    }
    pub fn with_role(mut self, node_id: &str, role: SemanticRole) -> Self {
        if let Some(node) = self.tree.nodes.iter_mut().find(|node| node.id == node_id) {
            node.semantic_role = Some(role);
        }
        self
    }

    /// Adds a summary agent node for each challenge observation so the agent
    /// tree surfaces visible human-verification walls. The observer is
    /// observation-only; this method does not solve, mutate, or inject.
    pub fn with_challenge_observations(mut self, observations: &ChallengeObservationSet) -> Self {
        for observation in &observations.observations {
            let node_id = format!(
                "challenge:{}:{}",
                observation.provider,
                self.tree.nodes.len()
            );
            let name = format!(
                "challenge:{} ({:?})",
                observation.provider, observation.kind
            );
            let kind_label = challenge_kind_label(&observation.kind);
            self.tree.nodes.push(AgentNode {
                id: node_id.clone(),
                origin: Some(observation.url.as_str().to_string()),
                identity_key: Some(format!("challenge:{}", observation.provider)),
                structural_role: StructuralRole::Region,
                semantic_role: Some(SemanticRole::Challenge),
                application_type: None,
                name: Some(name),
                value: Some(browsai_agent_tree::AgentValue::Text(kind_label.into())),
                description: Some(format!(
                    "{} challenge detected on {}",
                    observation.provider, observation.url
                )),
                state: browsai_agent_tree::NodeState::default(),
                geometry: None,
                relationships: Vec::new(),
                actions: Vec::new(),
                children: Vec::new(),
                provenance: observation.provenance.clone(),
                confidence: browsai_provenance::Confidence(observation.confidence),
                generation: self.tree.generation,
            });
        }
        self
    }

    /// Returns the providers of any challenge nodes in the agent tree.
    pub fn challenge_providers(&self) -> Vec<String> {
        let mut providers = Vec::new();
        for node in &self.tree.nodes {
            if matches!(node.semantic_role, Some(SemanticRole::Challenge)) {
                if let Some(provider) = node
                    .identity_key
                    .as_deref()
                    .and_then(|key| key.strip_prefix("challenge:"))
                {
                    if !providers.iter().any(|p| p == provider) {
                        providers.push(provider.to_string());
                    }
                }
            }
        }
        providers
    }

    pub fn challenge_present(&self) -> bool {
        self.tree
            .nodes
            .iter()
            .any(|node| matches!(node.semantic_role, Some(SemanticRole::Challenge)))
    }

    /// Adds a summary agent node for each fingerprint inconsistency so the
    /// agent tree surfaces the mismatch instead of silently shipping it.
    pub fn with_profile_inconsistencies(
        mut self,
        fingerprint_id: &str,
        inconsistencies: &[FingerprintInconsistency],
    ) -> Self {
        for inconsistency in inconsistencies {
            let node_id = format!(
                "profile-inconsistency:{}:{}",
                fingerprint_id,
                self.tree.nodes.len()
            );
            let description = format!("{inconsistency}");
            self.tree.nodes.push(AgentNode {
                id: node_id,
                origin: None,
                identity_key: Some(format!("profile-inconsistency:{fingerprint_id}")),
                structural_role: StructuralRole::Region,
                semantic_role: Some(SemanticRole::ProfileInconsistency),
                application_type: None,
                name: Some(format!("profile-inconsistency:{}", fingerprint_id)),
                value: Some(browsai_agent_tree::AgentValue::Text(description.clone())),
                description: Some(description),
                state: browsai_agent_tree::NodeState::default(),
                geometry: None,
                relationships: Vec::new(),
                actions: Vec::new(),
                children: Vec::new(),
                provenance: vec![browsai_provenance::ProvenanceSource {
                    kind: SourceKind::AgentInference,
                    reference: "semantic-ir".into(),
                    detail: None,
                }],
                confidence: browsai_provenance::Confidence(0.95),
                generation: self.tree.generation,
            });
        }
        self
    }

    pub fn profile_inconsistency_present(&self) -> bool {
        self.tree
            .nodes
            .iter()
            .any(|node| matches!(node.semantic_role, Some(SemanticRole::ProfileInconsistency)))
    }
    pub fn node(&self, id: &str) -> Option<&AgentNode> {
        self.tree.find(id)
    }

    /// Normalizes user-facing labels and reports deterministic resolution
    /// blockers without guessing at an ambiguous or unnamed target.
    pub fn normalize(&mut self) -> SemanticNormalization {
        let mut result = SemanticNormalization::default();
        for node in &mut self.tree.nodes {
            if let Some(name) = &mut node.name {
                let normalized = name.split_whitespace().collect::<Vec<_>>().join(" ");
                if normalized.is_empty() {
                    node.name = None;
                    result.changed_nodes.push(node.id.clone());
                } else if *name != normalized {
                    *name = normalized;
                    result.changed_nodes.push(node.id.clone());
                }
            }
            if node.name.is_none() && requires_accessible_name(&node.structural_role) {
                result.issues.push(SemanticIssue::MissingAccessibleName {
                    node_id: node.id.clone(),
                    structural_role: node.structural_role.clone(),
                });
            }
        }

        let mut identities: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for node in &self.tree.nodes {
            if let Some(identity) = &node.identity_key {
                identities
                    .entry(identity.clone())
                    .or_default()
                    .push(node.id.clone());
            }
        }
        for (identity_key, node_ids) in identities {
            if node_ids.len() > 1 {
                result.issues.push(SemanticIssue::AmbiguousIdentity {
                    identity_key,
                    node_ids,
                });
            }
        }
        result
    }

    pub fn validate_golden(&self, fixture: &SemanticGoldenFixture) -> Vec<String> {
        let mut mismatches = Vec::new();
        let mut actual_roles = self
            .tree
            .nodes
            .iter()
            .map(|node| node.structural_role.clone())
            .collect::<Vec<_>>();
        let mut expected_roles = fixture.roles.clone();
        actual_roles.sort_by_key(|role| format!("{role:?}"));
        expected_roles.sort_by_key(|role| format!("{role:?}"));
        if actual_roles != expected_roles {
            mismatches.push("structural roles differ".into());
        }
        let mut actual_labels = self
            .tree
            .nodes
            .iter()
            .filter_map(|node| node.name.clone())
            .collect::<Vec<_>>();
        let mut expected_labels = fixture.labels.clone();
        actual_labels.sort();
        expected_labels.sort();
        if actual_labels != expected_labels {
            mismatches.push("labels differ".into());
        }
        let mut actual_relationships = self
            .tree
            .nodes
            .iter()
            .flat_map(|node| {
                node.relationships
                    .iter()
                    .map(|relationship| relationship.kind.clone())
            })
            .collect::<Vec<_>>();
        let mut expected_relationships = fixture.relationship_kinds.clone();
        actual_relationships.sort();
        expected_relationships.sort();
        if actual_relationships != expected_relationships {
            mismatches.push("relationships differ".into());
        }
        if self
            .tree
            .nodes
            .iter()
            .any(|node| node.state.visible != fixture.visible)
        {
            mismatches.push("visibility differs".into());
        }
        if self
            .tree
            .nodes
            .iter()
            .any(|node| node.state.enabled != fixture.enabled)
        {
            mismatches.push("enabled state differs".into());
        }
        let geometry_count = self
            .tree
            .nodes
            .iter()
            .filter(|node| node.geometry.is_some())
            .count();
        if geometry_count != fixture.geometry_count {
            mismatches.push("geometry differs".into());
        }
        if self
            .tree
            .nodes
            .iter()
            .any(|node| (node.confidence.0 - fixture.confidence).abs() > f32::EPSILON)
        {
            mismatches.push("confidence differs".into());
        }
        let actual_provenance = self
            .tree
            .nodes
            .iter()
            .flat_map(|node| node.provenance.iter().map(|source| source.kind.clone()))
            .map(|kind| format!("{kind:?}"))
            .collect::<BTreeSet<_>>();
        let expected_provenance = fixture
            .provenance
            .iter()
            .map(|kind| format!("{kind:?}"))
            .collect::<BTreeSet<_>>();
        if actual_provenance != expected_provenance {
            mismatches.push("provenance differs".into());
        }
        mismatches
    }
}

fn challenge_kind_label(kind: &ChallengeKind) -> &'static str {
    match kind {
        ChallengeKind::Captcha => "captcha",
        ChallengeKind::BrowserChallenge => "browser-challenge",
        ChallengeKind::AccessDenied => "access-denied",
        ChallengeKind::TwoFactorPrompt => "two-factor",
        ChallengeKind::AccountLocked => "account-locked",
        ChallengeKind::TermsOfServiceGate => "tos-gate",
    }
}

fn requires_accessible_name(role: &StructuralRole) -> bool {
    matches!(
        role,
        StructuralRole::Button
            | StructuralRole::Link
            | StructuralRole::Textbox
            | StructuralRole::Checkbox
            | StructuralRole::Radio
            | StructuralRole::Select
    )
}
