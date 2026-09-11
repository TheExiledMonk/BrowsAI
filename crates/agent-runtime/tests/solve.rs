use browsai_agent_runtime::{
    AgentRuntime, AgentRuntimeError, TakeoverManager, TakeoverState,
};
use browsai_input::{MouseTrajectoryOptions, NativeInputEvent};
use browsai_sandbox::{Capability, SandboxPolicy};

fn policy_with_solve() -> SandboxPolicy {
    let mut policy = SandboxPolicy::agent();
    policy.capabilities.insert(Capability::SolveChallenge);
    policy
}

#[test]
fn solve_requires_active_human_takeover() {
    let mut runtime = AgentRuntime::new(policy_with_solve());
    let session = runtime.open(
        [Capability::SolveChallenge],
        Default::default(),
    );
    runtime.start(session).unwrap();
    let mut takeovers = TakeoverManager::default();
    let tid = takeovers.request("agent-a", "challenge: hcaptcha", 50, 10);
    assert_eq!(
        runtime.attempt_solve_challenge(session, "hcaptcha", Some(tid), &takeovers, 11),
        Err(AgentRuntimeError::CapabilityDenied(
            Capability::SolveChallenge
        ))
    );
    takeovers.accept(tid, 12);
    takeovers.pause(tid, 13);
    assert_eq!(
        runtime.attempt_solve_challenge(session, "hcaptcha", Some(tid), &takeovers, 14),
        Err(AgentRuntimeError::CapabilityDenied(
            Capability::SolveChallenge
        ))
    );
    takeovers.resume(tid, 15);
    let event = runtime
        .attempt_solve_challenge(session, "hcaptcha", Some(tid), &takeovers, 16)
        .unwrap();
    assert_eq!(event.provider, "hcaptcha");
    assert_eq!(event.capability_used, "SolveChallenge");
    let _ = TakeoverState::Active;
}

#[test]
fn solve_accepts_credential_solve_capability() {
    let mut runtime = AgentRuntime::new(policy_with_solve());
    let session = runtime.open(
        [Capability::SolveChallenge, Capability::CredentialSolve],
        Default::default(),
    );
    runtime.start(session).unwrap();
    let takeovers = TakeoverManager::default();
    let event = runtime
        .attempt_solve_challenge(session, "generic", None, &takeovers, 1)
        .unwrap();
    assert_eq!(event.capability_used, "CredentialSolve");
}

#[test]
fn solve_denied_without_capability() {
    let mut runtime = AgentRuntime::default();
    let session = runtime.open([Capability::Render], Default::default());
    runtime.start(session).unwrap();
    let takeovers = TakeoverManager::default();
    assert_eq!(
        runtime.attempt_solve_challenge(session, "hcaptcha", None, &takeovers, 0),
        Err(AgentRuntimeError::CapabilityDenied(
            Capability::SolveChallenge
        ))
    );
}

#[test]
fn unattended_policy_allows_solve_without_takeover() {
    let mut runtime = AgentRuntime::new(SandboxPolicy::autonomous_agent());
    let session = runtime.open([Capability::SolveChallenge], Default::default());
    runtime.start(session).unwrap();
    let takeovers = TakeoverManager::default();
    let event = runtime
        .attempt_solve_challenge(session, "hcaptcha", None, &takeovers, 0)
        .unwrap();
    assert_eq!(event.capability_used, "SolveChallenge+Unattended");
    assert!(event.takeover_id.is_none());
}

#[test]
fn unattended_policy_still_requires_solve_challenge_capability() {
    let mut runtime = AgentRuntime::new(SandboxPolicy::autonomous_agent());
    let session = runtime.open([Capability::Render], Default::default());
    runtime.start(session).unwrap();
    let takeovers = TakeoverManager::default();
    assert_eq!(
        runtime.attempt_solve_challenge(session, "hcaptcha", None, &takeovers, 0),
        Err(AgentRuntimeError::CapabilityDenied(
            Capability::SolveChallenge
        ))
    );
}

#[test]
fn unattended_takeover_denied_when_policy_disallows() {
    let mut runtime = AgentRuntime::new(policy_with_solve());
    let session = runtime.open([Capability::SolveChallenge], Default::default());
    runtime.start(session).unwrap();
    let takeovers = TakeoverManager::default();
    assert_eq!(
        runtime.attempt_solve_challenge(session, "hcaptcha", None, &takeovers, 0),
        Err(AgentRuntimeError::CapabilityDenied(
            Capability::SolveChallenge
        ))
    );
}

#[test]
fn challenge_click_requires_active_human_takeover_and_carries_target_metadata() {
    let mut runtime = AgentRuntime::new(policy_with_solve());
    let session = runtime.open([Capability::SolveChallenge], Default::default());
    runtime.start(session).unwrap();
    let mut takeovers = TakeoverManager::default();
    let tid = takeovers.request("agent-a", "challenge: recaptcha", 50, 10);
    takeovers.accept(tid, 11);
    let event = runtime
        .attempt_challenge_click(
            session,
            7,
            "recaptcha",
            Some("challenge:recaptcha:0".into()),
            Some(tid),
            &takeovers,
            12,
        )
        .unwrap();
    assert_eq!(event.provider, "recaptcha");
    assert_eq!(event.page_id, Some(7));
    assert_eq!(event.target_node_id.as_deref(), Some("challenge:recaptcha:0"));
    assert_eq!(event.capability_used, "SolveChallenge");
    assert!(event.takeover_id.is_some());
}

#[test]
fn challenge_click_denied_without_active_takeover() {
    let mut runtime = AgentRuntime::new(policy_with_solve());
    let session = runtime.open([Capability::SolveChallenge], Default::default());
    runtime.start(session).unwrap();
    let takeovers = TakeoverManager::default();
    assert_eq!(
        runtime.attempt_challenge_click(session, 1, "recaptcha", None, None, &takeovers, 0),
        Err(AgentRuntimeError::CapabilityDenied(
            Capability::SolveChallenge
        ))
    );
}

#[test]
fn challenge_click_with_trajectory_returns_move_sequence_and_click() {
    let mut runtime = AgentRuntime::new(policy_with_solve());
    let session = runtime.open([Capability::SolveChallenge], Default::default());
    runtime.start(session).unwrap();
    let mut takeovers = TakeoverManager::default();
    let tid = takeovers.request("agent-a", "challenge: hcaptcha", 50, 10);
    takeovers.accept(tid, 11);
    let (audit, trajectory) = runtime
        .attempt_challenge_click_with_trajectory(
            session,
            7,
            Some("challenge:hcaptcha:0".into()),
            "hcaptcha",
            (10.0, 20.0),
            (200.0, 220.0),
            Some(tid),
            &takeovers,
            12,
            MouseTrajectoryOptions::default(),
        )
        .unwrap();
    assert_eq!(audit.provider, "hcaptcha");
    assert_eq!(audit.page_id, Some(7));
    assert_eq!(
        audit.target_node_id.as_deref(),
        Some("challenge:hcaptcha:0")
    );
    let events = trajectory.events;
    assert!(events.len() >= 4);
    let moves: Vec<(f64, f64)> = events
        .iter()
        .filter_map(|e| match e {
            NativeInputEvent::PointerMove { x, y } => Some((*x, *y)),
            _ => None,
        })
        .collect();
    assert!(moves.len() >= 4);
    let (sx, sy) = moves[0];
    assert!((sx - 10.0).abs() < 1.0, "sx was {sx}");
    assert!((sy - 20.0).abs() < 1.0, "sy was {sy}");
    let (ex, ey) = *moves.last().unwrap();
    assert!((ex - 200.0).abs() < 5.0, "ex was {ex}");
    assert!((ey - 220.0).abs() < 5.0, "ey was {ey}");
    assert!(matches!(events[events.len() - 2], NativeInputEvent::PointerDown { .. }));
    assert!(matches!(events[events.len() - 1], NativeInputEvent::PointerUp { .. }));
}

#[test]
fn challenge_click_with_trajectory_is_not_a_straight_line() {
    let mut runtime = AgentRuntime::new(policy_with_solve());
    let session = runtime.open([Capability::SolveChallenge], Default::default());
    runtime.start(session).unwrap();
    let mut takeovers = TakeoverManager::default();
    let tid = takeovers.request("agent-a", "challenge: turnstile", 50, 10);
    takeovers.accept(tid, 11);
    let (_audit, trajectory) = runtime
        .attempt_challenge_click_with_trajectory(
            session,
            7,
            None,
            "turnstile",
            (0.0, 0.0),
            (600.0, 0.0),
            Some(tid),
            &takeovers,
            12,
            MouseTrajectoryOptions::default(),
        )
        .unwrap();
    let moves: Vec<(f64, f64)> = trajectory
        .events
        .iter()
        .filter_map(|e| match e {
            NativeInputEvent::PointerMove { x, y } => Some((*x, *y)),
            _ => None,
        })
        .collect();
    let midpoint = moves[moves.len() / 2];
    let max_y = moves
        .iter()
        .take(moves.len() - 1)
        .map(|(_, y)| y.abs())
        .fold(0.0_f64, f64::max);
    assert!(
        max_y > 5.0,
        "expected a curved trajectory, max |y| was {max_y}"
    );
    let _ = midpoint;
}

#[test]
fn challenge_click_with_trajectory_denied_without_active_takeover() {
    let mut runtime = AgentRuntime::new(policy_with_solve());
    let session = runtime.open([Capability::SolveChallenge], Default::default());
    runtime.start(session).unwrap();
    let takeovers = TakeoverManager::default();
    assert_eq!(
        runtime.attempt_challenge_click_with_trajectory(
            session,
            1,
            None,
            "hcaptcha",
            (0.0, 0.0),
            (100.0, 100.0),
            None,
            &takeovers,
            0,
            MouseTrajectoryOptions::default(),
        ),
        Err(AgentRuntimeError::CapabilityDenied(
            Capability::SolveChallenge
        ))
    );
}