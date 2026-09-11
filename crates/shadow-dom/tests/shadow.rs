use browsai_shadow_dom::{ShadowDomRegistry, ShadowRootMode};

#[test]
fn closed_shadow_roots_do_not_expose_internal_children_to_agents() {
    let mut registry = ShadowDomRegistry::default();
    let closed = registry.attach(1, ShadowRootMode::Closed);
    registry.add_child(&closed, 2);
    assert_eq!(registry.exposed_children(&closed, false), Some(vec![]));
    assert_eq!(registry.exposed_children(&closed, true), Some(vec![2]));
}

#[test]
fn shadow_attachment_and_children_are_idempotent_and_removable() {
    let mut registry = ShadowDomRegistry::default();
    let root = registry.attach(7, ShadowRootMode::Open);
    assert_eq!(registry.attach(7, ShadowRootMode::Closed), root);
    assert!(registry.add_child(&root, 10));
    assert!(!registry.add_child(&root, 10));
    assert_eq!(
        registry.root_for_host(7).unwrap().mode,
        ShadowRootMode::Open
    );
    assert!(registry.remove_child(&root, 10));
    assert!(!registry.remove_child(&root, 10));
    assert_eq!(registry.exposed_children(&root, false), Some(vec![]));
}

#[test]
fn composed_selectors_and_events_respect_closed_root_boundaries() {
    let mut registry = ShadowDomRegistry::default();
    let open = registry.attach(1, ShadowRootMode::Open);
    assert!(registry.add_child(&open, 2));
    assert!(registry.add_selector(&open, 2, ".save"));
    assert_eq!(
        registry.query_selector(&open, ".save", false),
        Some(vec![2])
    );

    let closed = registry.attach(3, ShadowRootMode::Closed);
    assert!(registry.add_child(&closed, 4));
    assert!(registry.add_selector(&closed, 4, ".secret"));
    assert_eq!(
        registry.query_selector(&closed, ".secret", false),
        Some(vec![])
    );
    assert_eq!(
        registry.query_selector(&closed, ".secret", true),
        Some(vec![4])
    );
    assert!(registry.drain_events().count() >= 4);
}
