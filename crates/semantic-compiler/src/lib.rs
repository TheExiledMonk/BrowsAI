//! First-pass semantic compilation from observed DOM and layout state.

use browsai_agent_tree::{
    ActionDescriptor, AgentNode, AgentNodeId, AgentRenderTree, Geometry, NodeState, SemanticRole,
    StructuralRole,
};
use browsai_dom_observer::{EngineNodeId, NodeKind, RawDocument};
use browsai_layout_observer::LayoutSnapshot;
use browsai_provenance::{Confidence, ProvenanceSource, SourceKind};
use browsai_runtime_observer::{RuntimeEvent, RuntimeEvidence};
use browsai_semantic_ir::{SemanticIr, SemanticNormalization};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default)]
pub struct DirtySubtrees {
    nodes: BTreeSet<EngineNodeId>,
}

impl DirtySubtrees {
    pub fn mark(&mut self, node: EngineNodeId) {
        self.nodes.insert(node);
    }
    pub fn contains(&self, node: EngineNodeId) -> bool {
        self.nodes.contains(&node)
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    pub fn drain(&mut self) -> impl Iterator<Item = EngineNodeId> + '_ {
        std::mem::take(&mut self.nodes).into_iter()
    }
}

#[derive(Clone, Debug)]
pub struct CompileResult {
    pub tree: AgentRenderTree,
    pub recomputed_nodes: Vec<EngineNodeId>,
}

#[derive(Default)]
pub struct SemanticCompiler {
    generation: u64,
}

impl SemanticCompiler {
    pub fn compile(&mut self, document: &RawDocument, layout: &LayoutSnapshot) -> CompileResult {
        self.generation += 1;
        let root_id = format!("page:{}", document.url);
        let mut tree = AgentRenderTree {
            root: root_id.clone(),
            nodes: vec![],
            generation: self.generation,
            truncated: false,
        };
        if let Some(root) = document.root {
            self.visit(document, layout, root, None, &mut tree);
        }
        CompileResult {
            tree,
            recomputed_nodes: document.nodes.keys().copied().collect(),
        }
    }

    /// Compiles the current observed state while reporting the dirty roots that
    /// caused recomputation. The compiler intentionally keeps the public API
    /// incremental now; subtree memoization will replace the traversal once
    /// engine mutation streams are connected.
    pub fn compile_dirty(
        &mut self,
        document: &RawDocument,
        layout: &LayoutSnapshot,
        dirty: &mut DirtySubtrees,
    ) -> CompileResult {
        let recomputed_nodes: Vec<_> = dirty.drain().collect();
        let mut result = self.compile(document, layout);
        if !recomputed_nodes.is_empty() {
            result.recomputed_nodes = recomputed_nodes;
        }
        result
    }

    /// Compiles and normalizes semantic output in one boundary operation.
    /// Ambiguous identities and missing accessible names are returned to the
    /// caller as data so query/action layers can refuse unsafe guesses.
    pub fn compile_normalized(
        &mut self,
        document: &RawDocument,
        layout: &LayoutSnapshot,
    ) -> (CompileResult, SemanticNormalization) {
        let mut result = self.compile(document, layout);
        let mut semantic = SemanticIr::from_structural(result.tree);
        let normalization = semantic.normalize();
        result.tree = semantic.tree;
        (result, normalization)
    }

    /// Compiles browser state and folds runtime evidence into the resulting
    /// agent tree. Runtime signals never replace DOM/layout facts; they add
    /// causal provenance and lower confidence when behavior, rather than
    /// static structure, is the source of the observation.
    pub fn compile_with_runtime(
        &mut self,
        document: &RawDocument,
        layout: &LayoutSnapshot,
        runtime: &[RuntimeEvidence],
    ) -> CompileResult {
        let mut result = self.compile(document, layout);
        for evidence in runtime {
            let target = match evidence.event {
                RuntimeEvent::DomMutation { node_id } => Some(format!("dom:{node_id}")),
                _ => None,
            };
            if let Some(target) = target {
                if let Some(node) = result.tree.nodes.iter_mut().find(|node| node.id == target) {
                    node.provenance.extend(evidence.provenance.clone());
                    node.confidence = Confidence::PROBABLE;
                }
            }
        }
        result
    }

    fn visit(
        &self,
        document: &RawDocument,
        layout: &LayoutSnapshot,
        id: EngineNodeId,
        parent: Option<AgentNodeId>,
        tree: &mut AgentRenderTree,
    ) {
        let raw = document
            .node(id)
            .expect("document traversal only visits existing nodes");
        let node_id = format!("dom:{id}");
        let (role, semantic, name) = classify(
            raw.kind.clone(),
            raw.name.as_deref(),
            &raw.attributes,
            raw.text.as_deref(),
        );
        let geometry = layout.get(id).map(|layout| Geometry {
            x: layout.rect.x,
            y: layout.rect.y,
            width: layout.rect.width,
            height: layout.rect.height,
        });
        let visible = layout
            .get(id)
            .map(|layout| layout.visible && !layout.clipped)
            .unwrap_or(true);
        let mut node = AgentNode {
            id: node_id.clone(),
            origin: None,
            identity_key: stable_identity(raw, &role),
            structural_role: role,
            semantic_role: semantic,
            application_type: None,
            name,
            value: raw.text.clone().map(browsai_agent_tree::AgentValue::Text),
            description: None,
            state: NodeState {
                visible,
                enabled: !layout
                    .get(id)
                    .map(|layout| layout.disabled)
                    .unwrap_or(false),
                ..Default::default()
            },
            geometry,
            relationships: vec![],
            actions: vec![],
            children: vec![],
            provenance: vec![ProvenanceSource {
                kind: SourceKind::Dom,
                reference: id.to_string(),
                detail: raw.name.clone(),
            }],
            confidence: Confidence::DIRECT,
            generation: tree.generation,
        };
        if matches!(node.semantic_role, Some(SemanticRole::SubmitAction)) {
            node.actions.push(ActionDescriptor {
                name: "activate".into(),
                consequence: "UNKNOWN".into(),
            });
        }
        if let Some(parent_id) = parent {
            if let Some(parent_node) = tree
                .nodes
                .iter_mut()
                .find(|candidate| candidate.id == parent_id)
            {
                parent_node.children.push(node_id.clone());
            }
        }
        tree.nodes.push(node);
        for child in &raw.children {
            self.visit(document, layout, *child, Some(node_id.clone()), tree);
        }
    }
}

fn classify(
    kind: NodeKind,
    tag: Option<&str>,
    attributes: &std::collections::BTreeMap<String, String>,
    text: Option<&str>,
) -> (StructuralRole, Option<SemanticRole>, Option<String>) {
    if kind == NodeKind::Document {
        return (StructuralRole::Page, None, None);
    }
    if kind == NodeKind::Frame {
        return (StructuralRole::Frame, None, tag.map(str::to_owned));
    }
    if kind == NodeKind::ShadowRoot {
        return (StructuralRole::ShadowRoot, None, None);
    }
    let tag = tag.unwrap_or_default().to_ascii_lowercase();
    let role = match tag.as_str() {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => StructuralRole::Heading,
        "p" => StructuralRole::Paragraph,
        "button" => StructuralRole::Button,
        "a" => StructuralRole::Link,
        "input" => StructuralRole::Textbox,
        "form" => StructuralRole::Form,
        "main" | "section" | "nav" => StructuralRole::Region,
        "img" => StructuralRole::Image,
        "dialog" => StructuralRole::Dialog,
        _ if kind == NodeKind::Text => StructuralRole::Text,
        _ => StructuralRole::Unknown,
    };
    let name = attributes
        .get("aria-label")
        .cloned()
        .or_else(|| attributes.get("alt").cloned())
        .or_else(|| text.map(str::to_owned));
    let accessible_name = name.as_deref().unwrap_or_default().to_ascii_lowercase();
    let semantic = if attributes
        .get("type")
        .map(|value| value == "submit")
        .unwrap_or(false)
        || tag == "button" && accessible_name.contains("save")
    {
        Some(SemanticRole::SubmitAction)
    } else if attributes
        .get("type")
        .map(|value| value == "search")
        .unwrap_or(false)
    {
        Some(SemanticRole::SearchField)
    } else {
        None
    };
    (role, semantic, name)
}

fn stable_identity(raw: &browsai_dom_observer::RawNode, role: &StructuralRole) -> Option<String> {
    raw.attributes
        .get("data-key")
        .map(|value| format!("{}:{}", role_name(role), value))
        .or_else(|| {
            raw.attributes
                .get("id")
                .map(|value| format!("{}#{}", role_name(role), value))
        })
        .or_else(|| {
            raw.attributes
                .get("aria-label")
                .map(|value| format!("{}:{}", role_name(role), value.to_ascii_lowercase()))
        })
}

fn role_name(role: &StructuralRole) -> &'static str {
    match role {
        StructuralRole::Page => "page",
        StructuralRole::Region => "region",
        StructuralRole::Heading => "heading",
        StructuralRole::Paragraph => "paragraph",
        StructuralRole::Text => "text",
        StructuralRole::Button => "button",
        StructuralRole::Link => "link",
        StructuralRole::Textbox => "textbox",
        StructuralRole::Checkbox => "checkbox",
        StructuralRole::Radio => "radio",
        StructuralRole::Select => "select",
        StructuralRole::List => "list",
        StructuralRole::ListItem => "list_item",
        StructuralRole::Table => "table",
        StructuralRole::Row => "row",
        StructuralRole::Cell => "cell",
        StructuralRole::Dialog => "dialog",
        StructuralRole::Menu => "menu",
        StructuralRole::Tab => "tab",
        StructuralRole::Image => "image",
        StructuralRole::Canvas => "canvas",
        StructuralRole::Video => "video",
        StructuralRole::Form => "form",
        StructuralRole::Frame => "frame",
        StructuralRole::ShadowRoot => "shadow_root",
        StructuralRole::Unknown => "unknown",
    }
}
