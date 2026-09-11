use browsai_action_planner::{ActionHistory, ActionPlanner, PlanningError};
use browsai_agent_tree::{AgentNode, AgentRenderTree, Geometry, NodeState, StructuralRole};
use browsai_input::{ActionType, AgentAction, NativeInputEvent};
use browsai_layout_observer::{LayoutBox, LayoutSnapshot};
use browsai_provenance::Confidence;
use serde_json::json;

fn tree_with_node(id: &str, visible: bool) -> AgentRenderTree {
    let mut tree = AgentRenderTree::new_page("Example");
    tree.nodes.push(AgentNode {
        id: id.into(),
        origin: None,
        identity_key: None,
        structural_role: StructuralRole::Unknown,
        semantic_role: None,
        application_type: None,
        name: Some(id.into()),
        value: None,
        description: None,
        state: NodeState {
            visible,
            enabled: true,
            ..Default::default()
        },
        geometry: Some(Geometry {
            x: 10.0,
            y: 20.0,
            width: 40.0,
            height: 20.0,
        }),
        relationships: vec![],
        actions: vec![],
        children: vec![],
        provenance: vec![],
        confidence: Confidence::DIRECT,
        generation: 0,
    });
    tree
}

#[test]
fn click_resolves_to_native_pointer_sequence() {
    let mut tree = AgentRenderTree::new_page("Example");
    tree.nodes.push(AgentNode {
        id: "dom:2".into(),
        origin: None,
        identity_key: Some("button:save".into()),
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
            y: 20.0,
            width: 40.0,
            height: 20.0,
        }),
        relationships: vec![],
        actions: vec![],
        children: vec![],
        provenance: vec![],
        confidence: Confidence::DIRECT,
        generation: 0,
    });
    let action = AgentAction {
        id: "a1".into(),
        target: "dom:2".into(),
        action_type: ActionType::Click,
        parameters: json!({}),
    };
    let plan = ActionPlanner::default()
        .plan(&tree, &LayoutSnapshot::default(), action)
        .unwrap();
    assert_eq!(plan.events.len(), 3);
}

#[test]
fn submit_requires_confirmation_before_dispatch() {
    let tree = AgentRenderTree::new_page("Example");
    let action = AgentAction {
        id: "a1".into(),
        target: tree.root.clone(),
        action_type: ActionType::Submit,
        parameters: json!({}),
    };
    assert!(matches!(
        ActionPlanner::default().plan(&tree, &LayoutSnapshot::default(), action),
        Err(PlanningError::ConfirmationRequired(_))
    ));
}

#[test]
fn drag_and_drop_resolve_to_coordinate_validated_pointer_events() {
    let tree = tree_with_node("source", true);
    let planner = ActionPlanner::default();
    let drag = planner
        .plan(
            &tree,
            &LayoutSnapshot::default(),
            AgentAction {
                id: "drag".into(),
                target: "source".into(),
                action_type: ActionType::Drag,
                parameters: serde_json::json!({"to_x": 300.0, "to_y": 400.0}),
            },
        )
        .unwrap();
    assert!(matches!(
        drag.events.as_slice(),
        [
            NativeInputEvent::PointerMove { .. },
            NativeInputEvent::PointerDown { button: 0 },
            NativeInputEvent::PointerMove { x: 300.0, y: 400.0 },
            NativeInputEvent::PointerUp { button: 0 }
        ]
    ));
    let missing_destination = planner.plan(
        &tree,
        &LayoutSnapshot::default(),
        AgentAction {
            id: "drag-missing".into(),
            target: "source".into(),
            action_type: ActionType::Drag,
            parameters: serde_json::json!({}),
        },
    );
    assert_eq!(missing_destination, Err(PlanningError::MissingCoordinates));
}

#[test]
fn focus_and_blur_resolve_to_native_browser_events() {
    let tree = tree_with_node("field", true);
    let planner = ActionPlanner::default();
    let focused = planner
        .plan(
            &tree,
            &LayoutSnapshot::default(),
            AgentAction {
                id: "focus".into(),
                target: "field".into(),
                action_type: ActionType::Focus,
                parameters: json!({}),
            },
        )
        .unwrap();
    assert!(matches!(
        focused.events.as_slice(),
        [
            NativeInputEvent::PointerMove { .. },
            NativeInputEvent::PointerDown { button: 0 },
            NativeInputEvent::PointerUp { button: 0 }
        ]
    ));
    let blurred = planner
        .plan(
            &tree,
            &LayoutSnapshot::default(),
            AgentAction {
                id: "blur".into(),
                target: "field".into(),
                action_type: ActionType::Blur,
                parameters: json!({}),
            },
        )
        .unwrap();
    assert_eq!(
        blurred.events,
        vec![
            NativeInputEvent::KeyDown { key: "Tab".into() },
            NativeInputEvent::KeyUp { key: "Tab".into() },
        ]
    );
}

#[test]
fn toggle_and_replace_text_preserve_native_intent() {
    let tree = tree_with_node("control", true);
    let planner = ActionPlanner::default();
    let toggle = planner
        .plan(
            &tree,
            &LayoutSnapshot::default(),
            AgentAction {
                id: "toggle".into(),
                target: "control".into(),
                action_type: ActionType::Toggle,
                parameters: json!({}),
            },
        )
        .unwrap();
    assert_eq!(toggle.events.len(), 3);
    let replace = planner
        .plan(
            &tree,
            &LayoutSnapshot::default(),
            AgentAction {
                id: "replace".into(),
                target: "control".into(),
                action_type: ActionType::ReplaceText,
                parameters: json!({"text": "new value"}),
            },
        )
        .unwrap();
    assert_eq!(
        replace.events,
        vec![
            NativeInputEvent::KeyDown {
                key: "Control".into()
            },
            NativeInputEvent::KeyDown { key: "A".into() },
            NativeInputEvent::KeyUp { key: "A".into() },
            NativeInputEvent::KeyUp {
                key: "Control".into()
            },
            NativeInputEvent::TextInput {
                text: "new value".into()
            }
        ]
    );
}

#[test]
fn hover_and_keypress_resolve_to_validated_native_events() {
    let tree = tree_with_node("control", true);
    let planner = ActionPlanner::default();
    let hover = planner
        .plan(
            &tree,
            &LayoutSnapshot::default(),
            AgentAction {
                id: "hover".into(),
                target: "control".into(),
                action_type: ActionType::Hover,
                parameters: json!({}),
            },
        )
        .unwrap();
    assert_eq!(
        hover.events,
        vec![NativeInputEvent::PointerMove { x: 30.0, y: 30.0 }]
    );

    let keypress = planner
        .plan(
            &tree,
            &LayoutSnapshot::default(),
            AgentAction {
                id: "keypress".into(),
                target: "control".into(),
                action_type: ActionType::KeyPress,
                parameters: json!({"key": "Enter"}),
            },
        )
        .unwrap();
    assert_eq!(
        keypress.events,
        vec![
            NativeInputEvent::KeyDown {
                key: "Enter".into()
            },
            NativeInputEvent::KeyUp {
                key: "Enter".into()
            },
        ]
    );
    assert_eq!(
        planner.plan(
            &tree,
            &LayoutSnapshot::default(),
            AgentAction {
                id: "missing-key".into(),
                target: "control".into(),
                action_type: ActionType::KeyPress,
                parameters: json!({}),
            },
        ),
        Err(PlanningError::MissingKey)
    );
}

#[test]
fn pointer_context_double_select_and_scroll_actions_preserve_native_fidelity() {
    let tree = tree_with_node("control", true);
    let planner = ActionPlanner::default();
    let context = planner
        .plan(
            &tree,
            &LayoutSnapshot::default(),
            AgentAction {
                id: "context".into(),
                target: "control".into(),
                action_type: ActionType::ContextClick,
                parameters: json!({}),
            },
        )
        .unwrap();
    assert!(context
        .events
        .iter()
        .any(|event| matches!(event, NativeInputEvent::PointerDown { button: 2 })));

    let double = planner
        .plan(
            &tree,
            &LayoutSnapshot::default(),
            AgentAction {
                id: "double".into(),
                target: "control".into(),
                action_type: ActionType::DoubleClick,
                parameters: json!({}),
            },
        )
        .unwrap();
    assert_eq!(
        double
            .events
            .iter()
            .filter(|event| matches!(event, NativeInputEvent::PointerDown { button: 0 }))
            .count(),
        2
    );

    let scroll = planner
        .plan(
            &tree,
            &LayoutSnapshot::default(),
            AgentAction {
                id: "scroll".into(),
                target: "control".into(),
                action_type: ActionType::Scroll,
                parameters: json!({"delta_y": 240.0}),
            },
        )
        .unwrap();
    assert_eq!(
        scroll.events,
        vec![NativeInputEvent::Scroll {
            delta_x: 0.0,
            delta_y: 240.0
        }]
    );
}

#[test]
fn action_history_retains_classification_and_replays_by_generation() {
    let tree = tree_with_node("control", true);
    let action = AgentAction {
        id: "history-1".into(),
        target: "control".into(),
        action_type: ActionType::Click,
        parameters: json!({}),
    };
    let planned = ActionPlanner::default()
        .plan(&tree, &LayoutSnapshot::default(), action.clone())
        .unwrap();
    let mut history = ActionHistory::default();
    history.record(&planned, tree.generation);

    assert_eq!(history.entries().len(), 1);
    assert_eq!(history.entries()[0].events.len(), 3);
    assert_eq!(history.replay("history-1", tree.generation), Some(action));
    assert_eq!(history.replay("history-1", tree.generation + 1), None);
    let audit = history.drain_audit();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].action_id, "history-1");
    assert_eq!(audit[0].target, "control");
    assert_eq!(audit[0].event_count, 3);
    assert!(history.drain_audit().is_empty());
    history.clear();
    assert!(history.entries().is_empty());
}

#[test]
fn planned_native_events_execute_through_dispatcher_and_return_result_state() {
    struct Dispatcher(Vec<NativeInputEvent>);
    impl browsai_input::NativeInputDispatcher for Dispatcher {
        type Error = ();

        fn dispatch(&mut self, event: NativeInputEvent) -> Result<(), Self::Error> {
            self.0.push(event);
            Ok(())
        }
    }

    let tree = tree_with_node("control", true);
    let planned = ActionPlanner::default()
        .plan(
            &tree,
            &LayoutSnapshot::default(),
            AgentAction {
                id: "execute".into(),
                target: "control".into(),
                action_type: ActionType::Click,
                parameters: json!({}),
            },
        )
        .unwrap();
    let mut dispatcher = Dispatcher(vec![]);
    let result = ActionPlanner::default()
        .execute(&planned, tree.generation, &mut dispatcher)
        .unwrap();
    assert!(result.completed);
    assert_eq!(result.dispatched_events, 3);
    assert_eq!(dispatcher.0.len(), 3);
    assert_eq!(result.generation, tree.generation);
}

#[test]
fn stale_and_ambiguous_references_are_rejected() {
    let mut tree = AgentRenderTree::new_page("Example");
    tree.generation = 4;
    let action = AgentAction {
        id: "a".into(),
        target: "button:save".into(),
        action_type: ActionType::Click,
        parameters: json!({}),
    };
    assert!(matches!(
        ActionPlanner::default().plan_at_generation(
            &tree,
            &LayoutSnapshot::default(),
            action.clone(),
            3
        ),
        Err(PlanningError::StaleReference { .. })
    ));
    tree.nodes.push(AgentNode {
        id: "one".into(),
        origin: None,
        identity_key: Some("button:save".into()),
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
            x: 1.0,
            y: 1.0,
            width: 10.0,
            height: 10.0,
        }),
        relationships: vec![],
        actions: vec![],
        children: vec![],
        provenance: vec![],
        confidence: Confidence::DIRECT,
        generation: 4,
    });
    tree.nodes.push(AgentNode {
        id: "two".into(),
        origin: None,
        identity_key: Some("button:save".into()),
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
            x: 20.0,
            y: 1.0,
            width: 10.0,
            height: 10.0,
        }),
        relationships: vec![],
        actions: vec![],
        children: vec![],
        provenance: vec![],
        confidence: Confidence::DIRECT,
        generation: 4,
    });
    assert_eq!(
        ActionPlanner::default().plan(&tree, &LayoutSnapshot::default(), action),
        Err(PlanningError::AmbiguousTarget("button:save".into()))
    );
}

#[test]
fn layout_hit_testing_rejects_an_overlapped_dom_target() {
    let mut tree = tree_with_node("dom:2", true);
    tree.nodes[1].geometry = Some(Geometry {
        x: 10.0,
        y: 10.0,
        width: 20.0,
        height: 20.0,
    });
    let mut layout = LayoutSnapshot::default();
    layout.insert(LayoutBox {
        node_id: 3,
        rect: browsai_layout_observer::Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        visible: true,
        clipped: false,
        z_index: 5,
        pointer_events: true,
        disabled: false,
        provenance: vec![],
    });
    let result = ActionPlanner::default().plan(
        &tree,
        &layout,
        AgentAction {
            id: "click".into(),
            target: "dom:2".into(),
            action_type: ActionType::Click,
            parameters: json!({}),
        },
    );
    assert_eq!(
        result,
        Err(PlanningError::TargetNotInteractable("dom:2".into()))
    );
}
