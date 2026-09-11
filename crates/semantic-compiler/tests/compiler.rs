use browsai_agent_tree::{SemanticRole, StructuralRole};
use browsai_dom_observer::{DomMutation, RawDocument, RawNode};
use browsai_layout_observer::{LayoutBox, LayoutSnapshot, Rect};
use browsai_provenance::{ProvenanceSource, SourceKind};
use browsai_runtime_observer::{RuntimeEvent, RuntimeEvidence};
use browsai_semantic_compiler::{DirtySubtrees, SemanticCompiler};

#[test]
fn compiler_derives_meaning_from_raw_state_and_layout() {
    let mut document = RawDocument::new("https://example.test/");
    document
        .apply(DomMutation::Insert {
            node: RawNode::document(1),
        })
        .unwrap();
    document
        .apply(DomMutation::Insert {
            node: RawNode::element(2, "button", Some(1)),
        })
        .unwrap();
    document
        .apply(DomMutation::SetAttribute {
            node: 2,
            name: "aria-label".into(),
            value: "Save".into(),
        })
        .unwrap();
    document
        .apply(DomMutation::SetAttribute {
            node: 2,
            name: "id".into(),
            value: "save-button".into(),
        })
        .unwrap();
    let mut layout = LayoutSnapshot {
        viewport: Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        ..Default::default()
    };
    layout.insert(LayoutBox {
        node_id: 2,
        rect: Rect {
            x: 1.0,
            y: 2.0,
            width: 30.0,
            height: 10.0,
        },
        visible: true,
        clipped: false,
        z_index: 1,
        pointer_events: true,
        disabled: false,
        provenance: vec![],
    });
    let result = SemanticCompiler::default().compile(&document, &layout);
    let button = result
        .tree
        .nodes
        .iter()
        .find(|node| node.structural_role == StructuralRole::Button)
        .unwrap();
    assert_eq!(button.name.as_deref(), Some("Save"));
    assert_eq!(button.semantic_role, Some(SemanticRole::SubmitAction));
    assert_eq!(button.geometry.as_ref().unwrap().x, 1.0);
    assert_eq!(button.provenance[0].reference, "2");
    assert_eq!(button.identity_key.as_deref(), Some("button#save-button"));
    assert!(result
        .tree
        .nodes
        .iter()
        .all(|node| node.generation == result.tree.generation));
}

#[test]
fn dirty_compilation_reports_only_marked_roots() {
    let document = RawDocument::new("about:blank");
    let layout = LayoutSnapshot::default();
    let mut dirty = DirtySubtrees::default();
    dirty.mark(42);
    let result = SemanticCompiler::default().compile_dirty(&document, &layout, &mut dirty);
    assert_eq!(result.recomputed_nodes, vec![42]);
    assert!(dirty.is_empty());
}

#[test]
fn normalized_compilation_exposes_ambiguity_without_guessing() {
    let document = RawDocument::new("about:blank");
    let layout = LayoutSnapshot::default();
    let (result, normalization) =
        SemanticCompiler::default().compile_normalized(&document, &layout);
    assert!(normalization.issues.is_empty());
    assert_eq!(result.tree.generation, 1);
}

#[test]
fn compiler_folds_runtime_behavior_into_semantic_provenance() {
    let mut document = RawDocument::new("https://example.test/");
    document
        .apply(DomMutation::Insert {
            node: RawNode::document(1),
        })
        .unwrap();
    document
        .apply(DomMutation::Insert {
            node: RawNode::element(2, "button", Some(1)),
        })
        .unwrap();
    let runtime = [RuntimeEvidence {
        event: RuntimeEvent::DomMutation { node_id: 2 },
        provenance: vec![ProvenanceSource {
            kind: SourceKind::JavaScript,
            reference: "script:toggle-button".into(),
            detail: None,
        }],
    }];
    let result = SemanticCompiler::default().compile_with_runtime(
        &document,
        &LayoutSnapshot::default(),
        &runtime,
    );
    let button = result.tree.find("dom:2").unwrap();
    assert_eq!(button.confidence, browsai_provenance::Confidence::PROBABLE);
    assert!(button
        .provenance
        .iter()
        .any(|source| source.reference == "script:toggle-button"));
}
