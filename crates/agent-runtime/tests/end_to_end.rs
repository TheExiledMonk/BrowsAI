use browsai_action_planner::{ActionPlanner, PlanningError};
use browsai_agent_protocol::{LeaseManager, LeaseScope, PageQuery, Query};
use browsai_agent_runtime::{AgentLimits, AgentRuntime, TakeoverManager};
use browsai_agent_tree::{AgentNode, AgentRenderTree, Geometry, NodeState, StructuralRole};
use browsai_input::{ActionType, AgentAction, NativeInputEvent};
use browsai_layout_observer::LayoutSnapshot;
use browsai_provenance::Confidence;
use serde_json::json;

#[test]
fn agent_task_covers_query_plan_stale_state_interruption_and_multi_agent_locking() {
    let mut tree = AgentRenderTree::new_page("Example");
    tree.generation = 3;
    tree.nodes.push(AgentNode {
        id: "button:save".into(),
        origin: None,
        identity_key: Some("save".into()),
        structural_role: StructuralRole::Button,
        semantic_role: None,
        application_type: None,
        name: Some("Save".into()),
        value: None,
        description: None,
        state: NodeState {
            visible: true,
            enabled: true,
            ..Default::default()
        },
        geometry: Some(Geometry {
            x: 10.0,
            y: 10.0,
            width: 80.0,
            height: 24.0,
        }),
        relationships: vec![],
        actions: vec![],
        children: vec![],
        provenance: vec![],
        confidence: Confidence::DIRECT,
        generation: 3,
    });
    assert_eq!(
        PageQuery::new(&tree)
            .query(&Query {
                name: Some("save".into()),
                ..Default::default()
            })
            .len(),
        1
    );
    let action = AgentAction {
        id: "a1".into(),
        target: "button:save".into(),
        action_type: ActionType::Click,
        parameters: json!({}),
    };
    let plan = ActionPlanner::default()
        .plan_at_generation(&tree, &LayoutSnapshot::default(), action, 3)
        .unwrap();
    assert!(matches!(
        plan.events[0],
        NativeInputEvent::PointerMove { .. }
    ));
    assert!(matches!(
        ActionPlanner::default().plan_at_generation(
            &tree,
            &LayoutSnapshot::default(),
            AgentAction {
                id: "a2".into(),
                target: "button:save".into(),
                action_type: ActionType::Click,
                parameters: json!({})
            },
            2
        ),
        Err(PlanningError::StaleReference { .. })
    ));

    let mut runtime = AgentRuntime::default();
    let session = runtime.open(
        [browsai_sandbox::Capability::Render],
        AgentLimits::default(),
    );
    runtime.start(session).unwrap();
    runtime.cancel(session).unwrap();
    let mut locks = LeaseManager::default();
    let lease = locks
        .acquire("agent-a", LeaseScope::Page("tab-1".into()), 10, 0)
        .unwrap();
    assert!(locks
        .acquire("agent-b", LeaseScope::Page("tab-1".into()), 10, 0)
        .is_err());
    assert!(locks.release(lease.id, "agent-a").is_ok());
    let mut takeover = TakeoverManager::default();
    let takeover_id = takeover.request("agent-a", "manual review", 10, 0);
    assert!(takeover.accept(takeover_id, 1));
}
