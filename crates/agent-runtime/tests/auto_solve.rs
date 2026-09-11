use browsai_agent_runtime::{
    solve_observed_challenges, AgentRuntime, TakeoverManager, SOLVE_NEAR_THRESHOLD_PX,
};
use browsai_agent_tree::{
    AgentNode, AgentRenderTree, AgentValue, Geometry, NodeState, SemanticRole, StructuralRole,
};
use browsai_input::NativeInputEvent;
use browsai_provenance::Confidence;
use browsai_sandbox::{Capability, SandboxPolicy};
use browsai_state::PageSnapshot;

fn challenge_node(id: &str, x: f64, y: f64, w: f64, h: f64) -> AgentNode {
    AgentNode {
        id: id.into(),
        origin: None,
        identity_key: Some(format!("challenge:{id}")),
        structural_role: StructuralRole::Region,
        semantic_role: Some(SemanticRole::Challenge),
        application_type: None,
        name: Some(format!("challenge {id}")),
        value: Some(AgentValue::Text("captcha".into())),
        description: Some("captcha".into()),
        state: NodeState::default(),
        geometry: Some(Geometry {
            x,
            y,
            width: w,
            height: h,
        }),
        relationships: Vec::new(),
        actions: Vec::new(),
        children: Vec::new(),
        provenance: Vec::new(),
        confidence: Confidence(1.0),
        generation: 0,
    }
}

fn snapshot_with(nodes: Vec<AgentNode>) -> PageSnapshot {
    let mut tree = AgentRenderTree::new_page("https://example.test/");
    tree.nodes.extend(nodes);
    PageSnapshot {
        schema_version: 1,
        id: 0,
        url: "https://example.test/".parse().unwrap(),
        tree,
        focused_node: None,
        scroll_x: 0.0,
        scroll_y: 0.0,
        pending_network: 0,
        storage_generation: 0,
        semantic_generation: 0,
    }
}

#[test]
fn solve_observed_challenges_dispatches_via_supplied_closure() {
    let mut runtime = AgentRuntime::new(SandboxPolicy::autonomous_agent());
    let session = runtime.open([Capability::SolveChallenge], Default::default());
    runtime.start(session).unwrap();
    let takeovers = TakeoverManager::default();
    let snapshot = snapshot_with(vec![challenge_node("hcaptcha", 100.0, 200.0, 80.0, 80.0)]);
    let mut dispatched: Vec<NativeInputEvent> = Vec::new();
    let now = runtime.now_millis();
    let events = solve_observed_challenges(
        7,
        &snapshot,
        &mut runtime,
        session,
        &takeovers,
        now,
        (0.0, 0.0),
        |event| {
            dispatched.push(event.clone());
            Ok(())
        },
        |_error| {},
    )
    .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].provider, "hcaptcha");
    assert_eq!(events[0].page_id, Some(7));
    assert!(!dispatched.is_empty(), "no events dispatched to closure");
    let pointer_down_count = dispatched
        .iter()
        .filter(|e| matches!(e, NativeInputEvent::PointerDown { .. }))
        .count();
    let pointer_up_count = dispatched
        .iter()
        .filter(|e| matches!(e, NativeInputEvent::PointerUp { .. }))
        .count();
    assert_eq!(pointer_down_count, 1);
    assert_eq!(pointer_up_count, 1);
}

#[test]
fn solve_observed_challenges_dispatches_trajectory_when_cursor_is_far() {
    let mut runtime = AgentRuntime::new(SandboxPolicy::autonomous_agent());
    let session = runtime.open([Capability::SolveChallenge], Default::default());
    runtime.start(session).unwrap();
    let takeovers = TakeoverManager::default();
    let snapshot = snapshot_with(vec![challenge_node("recaptcha", 600.0, 320.0, 80.0, 80.0)]);
    let mut dispatched: Vec<NativeInputEvent> = Vec::new();
    let now = runtime.now_millis();
    let events = solve_observed_challenges(
        7,
        &snapshot,
        &mut runtime,
        session,
        &takeovers,
        now,
        (0.0, 0.0),
        |event| {
            dispatched.push(event.clone());
            Ok(())
        },
        |_error| {},
    )
    .unwrap();
    assert_eq!(events.len(), 1);
    let move_count = dispatched
        .iter()
        .filter(|e| matches!(e, NativeInputEvent::PointerMove { .. }))
        .count();
    assert!(
        move_count >= 2,
        "expected trajectory with multiple moves, got {move_count}"
    );
}

#[test]
fn solve_observed_challenges_uses_discrete_click_within_threshold() {
    let mut runtime = AgentRuntime::new(SandboxPolicy::autonomous_agent());
    let session = runtime.open([Capability::SolveChallenge], Default::default());
    runtime.start(session).unwrap();
    let takeovers = TakeoverManager::default();
    let snapshot = snapshot_with(vec![challenge_node("hcaptcha", 100.0, 200.0, 80.0, 80.0)]);
    let cursor_at_target = (140.0, 240.0); // center of the widget
    let distance_from_target = SOLVE_NEAR_THRESHOLD_PX;
    let cursor_within_threshold = (
        cursor_at_target.0 + distance_from_target - 1.0,
        cursor_at_target.1,
    );
    let mut dispatched: Vec<NativeInputEvent> = Vec::new();
    let now = runtime.now_millis();
    let events = solve_observed_challenges(
        7,
        &snapshot,
        &mut runtime,
        session,
        &takeovers,
        now,
        cursor_within_threshold,
        |event| {
            dispatched.push(event.clone());
            Ok(())
        },
        |_error| {},
    )
    .unwrap();
    assert_eq!(events.len(), 1);
    let move_count = dispatched
        .iter()
        .filter(|e| matches!(e, NativeInputEvent::PointerMove { .. }))
        .count();
    assert_eq!(
        move_count, 0,
        "cursor is within threshold; no move events expected"
    );
}

#[test]
fn solve_observed_challenges_tracks_cursor_across_iterations() {
    let mut runtime = AgentRuntime::new(SandboxPolicy::autonomous_agent());
    let session = runtime.open([Capability::SolveChallenge], Default::default());
    runtime.start(session).unwrap();
    let takeovers = TakeoverManager::default();
    let snapshot = snapshot_with(vec![
        challenge_node("hcaptcha", 100.0, 200.0, 80.0, 80.0),
        challenge_node("recaptcha", 800.0, 600.0, 80.0, 80.0),
    ]);
    let mut dispatched: Vec<NativeInputEvent> = Vec::new();
    let now = runtime.now_millis();
    let events = solve_observed_challenges(
        7,
        &snapshot,
        &mut runtime,
        session,
        &takeovers,
        now,
        (0.0, 0.0),
        |event| {
            dispatched.push(event.clone());
            Ok(())
        },
        |_error| {},
    )
    .unwrap();
    assert_eq!(events.len(), 2);
}

#[test]
fn solve_observed_challenges_invokes_on_failure_callback() {
    let mut runtime = AgentRuntime::new(SandboxPolicy::autonomous_agent());
    let session = runtime.open([Capability::SolveChallenge], Default::default());
    runtime.start(session).unwrap();
    let takeovers = TakeoverManager::default();
    let snapshot = snapshot_with(vec![challenge_node("hcaptcha", 100.0, 200.0, 80.0, 80.0)]);
    let mut failure_messages: Vec<String> = Vec::new();
    let now = runtime.now_millis();
    let events = solve_observed_challenges(
        7,
        &snapshot,
        &mut runtime,
        session,
        &takeovers,
        now,
        (0.0, 0.0),
        |_event| Err("engine unavailable".into()),
        |error| failure_messages.push(error.to_string()),
    )
    .unwrap();
    assert_eq!(events.len(), 1);
    assert!(!failure_messages.is_empty());
    assert!(failure_messages.iter().all(|m| m == "engine unavailable"));
}
