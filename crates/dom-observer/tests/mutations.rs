use browsai_dom_observer::{DocumentLifecycle, DomMutation, RawDocument, RawNode};

#[test]
fn raw_observer_preserves_engine_identity_and_mutation_generation() {
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
        .apply(DomMutation::Insert {
            node: RawNode::text(3, "Save", Some(2)),
        })
        .unwrap();

    assert_eq!(document.generation, 4);
    assert_eq!(document.node(2).unwrap().children, vec![3]);
    assert_eq!(document.node(2).unwrap().attributes["aria-label"], "Save");
}

#[test]
fn references_are_generation_bound_and_navigation_resets_document_ownership() {
    let mut document = RawDocument::new("https://example.test/one");
    document
        .apply(DomMutation::Insert {
            node: RawNode::document(1),
        })
        .unwrap();
    let reference = document.reference(1).unwrap();
    document.advance_lifecycle(DocumentLifecycle::Complete);
    assert_eq!(
        document.resolve(reference),
        Err(browsai_dom_observer::DomObserverError::StaleReference)
    );
    document.begin_navigation("https://example.test/two");
    assert_eq!(document.lifecycle, DocumentLifecycle::Loading);
    assert!(document.node(1).is_none());
}

#[test]
fn removing_a_node_removes_its_entire_descendant_subtree() {
    let mut document = RawDocument::new("https://example.test/");
    for node in [
        RawNode::document(1),
        RawNode::element(2, "section", Some(1)),
        RawNode::element(3, "button", Some(2)),
        RawNode::text(4, "Save", Some(3)),
    ] {
        document.apply(DomMutation::Insert { node }).unwrap();
    }
    document.apply(DomMutation::Remove { node: 2 }).unwrap();
    assert!(document.node(2).is_none());
    assert!(document.node(3).is_none());
    assert!(document.node(4).is_none());
    assert_eq!(document.node(1).unwrap().children, Vec::<u64>::new());
}

#[test]
fn event_listener_observations_are_raw_generation_bound_facts() {
    let mut document = RawDocument::new("https://example.test");
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
        .apply(DomMutation::AddEventListener {
            node: 2,
            event: "click".into(),
        })
        .unwrap();
    assert!(document.node(2).unwrap().event_listeners.contains("click"));
    document
        .apply(DomMutation::RemoveEventListener {
            node: 2,
            event: "click".into(),
        })
        .unwrap();
    assert!(document.node(2).unwrap().event_listeners.is_empty());
}
