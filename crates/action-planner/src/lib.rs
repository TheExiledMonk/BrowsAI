//! Resolves semantic intent into native browser input after state and policy
//! validation.

use browsai_agent_tree::{AgentNodeId, AgentRenderTree};
use browsai_input::{ActionType, AgentAction, NativeInputDispatcher, NativeInputEvent};
use browsai_layout_observer::LayoutSnapshot;
use browsai_transactions::{
    Consequence, PolicyDecision, Reversibility, TransactionClassification, TransactionPolicy,
};

#[derive(Clone, Debug, PartialEq)]
pub struct PlannedAction {
    pub action: AgentAction,
    pub events: Vec<NativeInputEvent>,
    pub classification: TransactionClassification,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionResult {
    pub action_id: String,
    pub target: String,
    pub generation: u64,
    pub dispatched_events: usize,
    pub completed: bool,
    pub classification: TransactionClassification,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionHistoryEntry {
    pub action: AgentAction,
    pub events: Vec<NativeInputEvent>,
    pub classification: TransactionClassification,
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionAuditEvent {
    pub action_id: String,
    pub target: String,
    pub generation: u64,
    pub classification: TransactionClassification,
    pub event_count: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionHistory {
    entries: Vec<ActionHistoryEntry>,
    audit: Vec<ActionAuditEvent>,
}

impl ActionHistory {
    pub fn record(&mut self, planned: &PlannedAction, generation: u64) {
        self.entries.push(ActionHistoryEntry {
            action: planned.action.clone(),
            events: planned.events.clone(),
            classification: planned.classification.clone(),
            generation,
        });
        self.audit.push(ActionAuditEvent {
            action_id: planned.action.id.clone(),
            target: planned.action.target.clone(),
            generation,
            classification: planned.classification.clone(),
            event_count: planned.events.len(),
        });
    }

    pub fn entries(&self) -> &[ActionHistoryEntry] {
        &self.entries
    }

    pub fn replay(&self, action_id: &str, generation: u64) -> Option<AgentAction> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.action.id == action_id && entry.generation == generation)
            .map(|entry| entry.action.clone())
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.audit.clear();
    }

    pub fn drain_audit(&mut self) -> Vec<ActionAuditEvent> {
        std::mem::take(&mut self.audit)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlanningError {
    TargetNotFound(AgentNodeId),
    AmbiguousTarget(String),
    StaleReference { expected: u64, actual: u64 },
    TargetNotInteractable(AgentNodeId),
    ConfirmationRequired(TransactionClassification),
    Denied(TransactionClassification),
    MissingText,
    MissingKey,
    MissingCoordinates,
}

#[derive(Default)]
pub struct ActionPlanner {
    pub policy: TransactionPolicy,
}

impl ActionPlanner {
    pub fn plan(
        &self,
        tree: &AgentRenderTree,
        layout: &LayoutSnapshot,
        action: AgentAction,
    ) -> Result<PlannedAction, PlanningError> {
        self.plan_internal(tree, layout, action, None)
    }

    pub fn plan_at_generation(
        &self,
        tree: &AgentRenderTree,
        layout: &LayoutSnapshot,
        action: AgentAction,
        expected_generation: u64,
    ) -> Result<PlannedAction, PlanningError> {
        if tree.generation != expected_generation {
            return Err(PlanningError::StaleReference {
                expected: expected_generation,
                actual: tree.generation,
            });
        }
        self.plan_internal(tree, layout, action, Some(expected_generation))
    }

    pub fn execute<D: NativeInputDispatcher>(
        &self,
        planned: &PlannedAction,
        generation: u64,
        dispatcher: &mut D,
    ) -> Result<ActionResult, D::Error> {
        for event in &planned.events {
            dispatcher.dispatch(event.clone())?;
        }
        Ok(ActionResult {
            action_id: planned.action.id.clone(),
            target: planned.action.target.clone(),
            generation,
            dispatched_events: planned.events.len(),
            completed: true,
            classification: planned.classification.clone(),
        })
    }

    fn plan_internal(
        &self,
        tree: &AgentRenderTree,
        layout: &LayoutSnapshot,
        action: AgentAction,
        _generation: Option<u64>,
    ) -> Result<PlannedAction, PlanningError> {
        let node = if let Some(node) = tree.find(&action.target) {
            node
        } else {
            let matches = tree.find_identity(&action.target);
            match matches.as_slice() {
                [node] => *node,
                [] => return Err(PlanningError::TargetNotFound(action.target.clone())),
                _ => return Err(PlanningError::AmbiguousTarget(action.target.clone())),
            }
        };
        if !node.state.visible || !node.state.enabled {
            return Err(PlanningError::TargetNotInteractable(node.id.clone()));
        }
        if !layout.boxes.is_empty() {
            if let (Some(engine_id), Some(geometry)) =
                (engine_node_id(&node.id), node.geometry.as_ref())
            {
                let x = geometry.x + geometry.width / 2.0;
                let y = geometry.y + geometry.height / 2.0;
                if layout.hit_test(x, y) != Some(engine_id) {
                    return Err(PlanningError::TargetNotInteractable(node.id.clone()));
                }
            }
        }
        let classification = classify_action(&self.policy, &action.action_type);
        match classification.decision {
            PolicyDecision::Deny => return Err(PlanningError::Denied(classification)),
            PolicyDecision::RequireConfirmation => {
                return Err(PlanningError::ConfirmationRequired(classification))
            }
            PolicyDecision::Allow => {}
        }
        let events = match action.action_type {
            ActionType::Click
            | ActionType::Activate
            | ActionType::DoubleClick
            | ActionType::ContextClick
            | ActionType::Toggle
            | ActionType::Select => {
                let geometry = node
                    .geometry
                    .as_ref()
                    .ok_or(PlanningError::MissingCoordinates)?;
                let x = geometry.x + geometry.width / 2.0;
                let y = geometry.y + geometry.height / 2.0;
                let button = if matches!(action.action_type, ActionType::ContextClick) {
                    2
                } else {
                    0
                };
                let mut events = vec![
                    NativeInputEvent::PointerMove { x, y },
                    NativeInputEvent::PointerDown { button },
                    NativeInputEvent::PointerUp { button },
                ];
                if matches!(action.action_type, ActionType::DoubleClick) {
                    events.extend([
                        NativeInputEvent::PointerDown { button: 0 },
                        NativeInputEvent::PointerUp { button: 0 },
                    ]);
                }
                events
            }
            ActionType::Hover => {
                let geometry = node
                    .geometry
                    .as_ref()
                    .ok_or(PlanningError::MissingCoordinates)?;
                vec![NativeInputEvent::PointerMove {
                    x: geometry.x + geometry.width / 2.0,
                    y: geometry.y + geometry.height / 2.0,
                }]
            }
            ActionType::Type => vec![NativeInputEvent::TextInput {
                text: action
                    .parameters
                    .get("text")
                    .and_then(|value| value.as_str())
                    .ok_or(PlanningError::MissingText)?
                    .into(),
            }],
            ActionType::KeyPress => {
                let key = action
                    .parameters
                    .get("key")
                    .and_then(|value| value.as_str())
                    .filter(|value| !value.is_empty())
                    .ok_or(PlanningError::MissingKey)?;
                vec![
                    NativeInputEvent::KeyDown { key: key.into() },
                    NativeInputEvent::KeyUp { key: key.into() },
                ]
            }
            ActionType::ReplaceText => {
                let text = action
                    .parameters
                    .get("text")
                    .and_then(|value| value.as_str())
                    .ok_or(PlanningError::MissingText)?;
                vec![
                    NativeInputEvent::KeyDown {
                        key: "Control".into(),
                    },
                    NativeInputEvent::KeyDown { key: "A".into() },
                    NativeInputEvent::KeyUp { key: "A".into() },
                    NativeInputEvent::KeyUp {
                        key: "Control".into(),
                    },
                    NativeInputEvent::TextInput { text: text.into() },
                ]
            }
            ActionType::Scroll => vec![NativeInputEvent::Scroll {
                delta_x: action
                    .parameters
                    .get("delta_x")
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0),
                delta_y: action
                    .parameters
                    .get("delta_y")
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0),
            }],
            ActionType::Drag => {
                let geometry = node
                    .geometry
                    .as_ref()
                    .ok_or(PlanningError::MissingCoordinates)?;
                let to_x = action
                    .parameters
                    .get("to_x")
                    .and_then(|value| value.as_f64())
                    .ok_or(PlanningError::MissingCoordinates)?;
                let to_y = action
                    .parameters
                    .get("to_y")
                    .and_then(|value| value.as_f64())
                    .ok_or(PlanningError::MissingCoordinates)?;
                let from_x = geometry.x + geometry.width / 2.0;
                let from_y = geometry.y + geometry.height / 2.0;
                vec![
                    NativeInputEvent::PointerMove {
                        x: from_x,
                        y: from_y,
                    },
                    NativeInputEvent::PointerDown { button: 0 },
                    NativeInputEvent::PointerMove { x: to_x, y: to_y },
                    NativeInputEvent::PointerUp { button: 0 },
                ]
            }
            ActionType::Drop => {
                let geometry = node
                    .geometry
                    .as_ref()
                    .ok_or(PlanningError::MissingCoordinates)?;
                let x = geometry.x + geometry.width / 2.0;
                let y = geometry.y + geometry.height / 2.0;
                vec![
                    NativeInputEvent::PointerMove { x, y },
                    NativeInputEvent::PointerUp { button: 0 },
                ]
            }
            ActionType::Focus => {
                let geometry = node
                    .geometry
                    .as_ref()
                    .ok_or(PlanningError::MissingCoordinates)?;
                vec![
                    NativeInputEvent::PointerMove {
                        x: geometry.x + geometry.width / 2.0,
                        y: geometry.y + geometry.height / 2.0,
                    },
                    NativeInputEvent::PointerDown { button: 0 },
                    NativeInputEvent::PointerUp { button: 0 },
                ]
            }
            ActionType::Blur => vec![
                NativeInputEvent::KeyDown { key: "Tab".into() },
                NativeInputEvent::KeyUp { key: "Tab".into() },
            ],
            _ => vec![],
        };
        Ok(PlannedAction {
            action,
            events,
            classification,
        })
    }
}

fn engine_node_id(node_id: &str) -> Option<u64> {
    node_id.strip_prefix("dom:")?.parse().ok()
}

fn classify_action(policy: &TransactionPolicy, action: &ActionType) -> TransactionClassification {
    let (consequence, reversibility) = match action {
        ActionType::Navigate | ActionType::Back | ActionType::Forward | ActionType::Reload => {
            (Consequence::Navigate, Reversibility::LocalReversible)
        }
        ActionType::Type | ActionType::ReplaceText | ActionType::Toggle | ActionType::Select => {
            (Consequence::LocalWrite, Reversibility::LocalReversible)
        }
        ActionType::Submit | ActionType::Upload => (Consequence::Unknown, Reversibility::Unknown),
        ActionType::Download => (Consequence::LocalWrite, Reversibility::LocalReversible),
        ActionType::Authenticate => (Consequence::Authenticate, Reversibility::Unknown),
        _ => (Consequence::Read, Reversibility::LocalReversible),
    };
    policy.classify(consequence, reversibility)
}
