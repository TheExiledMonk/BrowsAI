use browsai_css_observer::{ComputedStyle, StyleSnapshot, VisibilityState};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Golden {
    node_id: u64,
    display: String,
    visibility: String,
    opacity: f32,
    z_index: i32,
    pointer_events: bool,
    clipped: bool,
    expected_state: String,
}

#[test]
fn css_visibility_matches_repository_golden_fixture() {
    let golden: Golden = serde_json::from_str(include_str!(
        "../../../test-sites/semantic-golden/css-visibility.json"
    ))
    .unwrap();
    let mut snapshot = StyleSnapshot {
        generation: 0,
        nodes: BTreeMap::new(),
        dirty_nodes: vec![],
    };
    snapshot.set(
        golden.node_id,
        ComputedStyle {
            display: golden.display,
            visibility: golden.visibility,
            opacity: golden.opacity,
            z_index: golden.z_index,
            pointer_events: golden.pointer_events,
            clipped: golden.clipped,
            ..Default::default()
        },
    );
    assert_eq!(
        format!(
            "{:?}",
            snapshot.get(golden.node_id).unwrap().visibility_state()
        ),
        golden.expected_state
    );
    assert!(snapshot.visible(golden.node_id));
}

#[test]
fn css_golden_states_remain_typed() {
    let style = ComputedStyle {
        display: "none".into(),
        ..Default::default()
    };
    assert_eq!(style.visibility_state(), VisibilityState::DisplayNone);
}
