use browsai_dom_observer::{RawDocument, RawNode};
use browsai_provenance::{ProvenanceSource, SourceKind};

#[test]
fn raw_dom_nodes_preserve_engine_source_provenance() {
    let mut document = RawDocument::new("https://example.test/");
    let mut node = RawNode::element(1, "button", None);
    node.provenance.push(ProvenanceSource {
        kind: SourceKind::Dom,
        reference: "engine:document-1/node-1".into(),
        detail: Some("parsed HTML element".into()),
    });
    document
        .apply(browsai_dom_observer::DomMutation::Insert { node })
        .unwrap();
    let encoded = serde_json::to_string(&document).unwrap();
    assert!(encoded.contains("engine:document-1/node-1"));
    assert_eq!(
        document.node(1).unwrap().provenance[0].kind,
        SourceKind::Dom
    );
}
