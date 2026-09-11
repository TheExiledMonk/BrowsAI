use browsai_dom_observer::{NodeKind, RawNode};
use std::collections::BTreeMap;

#[test]
fn raw_nodes_observe_forms_dialogs_frames_shadow_canvas_media_and_accessibility() {
    assert!(RawNode::element(1, "form", None).observed_signals().form);
    assert!(
        RawNode::element(1, "dialog", None)
            .observed_signals()
            .dialog
    );
    assert!(RawNode::element(1, "iframe", None).observed_signals().frame);
    assert!(
        RawNode::element(1, "canvas", None)
            .observed_signals()
            .canvas
    );
    assert_eq!(
        RawNode::element(1, "video", None).observed_signals().media,
        Some("video".into())
    );
    let shadow = RawNode {
        id: 2,
        kind: NodeKind::ShadowRoot,
        name: None,
        attributes: BTreeMap::new(),
        text: None,
        parent: None,
        children: vec![],
        event_listeners: Default::default(),
        provenance: vec![],
    };
    assert!(shadow.observed_signals().shadow_root);

    let mut accessible = RawNode::element(3, "button", None);
    accessible.attributes.insert("role".into(), "button".into());
    accessible
        .attributes
        .insert("aria-label".into(), "Save".into());
    accessible
        .attributes
        .insert("data-focused".into(), "true".into());
    assert_eq!(
        accessible.observed_signals().accessibility_role.as_deref(),
        Some("button")
    );
    assert_eq!(
        accessible.observed_signals().accessible_name.as_deref(),
        Some("Save")
    );
    assert!(accessible.observed_signals().focused);
}
