use browsai_state::StabilityState;

#[test]
fn page_stability_requires_no_work_and_a_quiet_period() {
    let mut state = StabilityState {
        pending_network: 0,
        pending_tasks: 0,
        last_mutation_generation: 1,
        quiet_generations: 0,
    };
    assert!(!state.is_stable(2));
    state.tick_quiet();
    state.tick_quiet();
    assert!(state.is_stable(2));
    state.mutation(2);
    assert!(!state.is_stable(1));
}

#[test]
fn runtime_state_tracks_normalized_values_time_history_and_locks() {
    let mut state = browsai_state::PageRuntimeState::default();
    state.set_value("total", "42.00");
    state.record_autocomplete("42.00", 2);
    state.record_autocomplete("42.01", 2);
    state.record_autocomplete("42.02", 2);
    state.focus(Some("dom:input".into()));
    state.scroll_to(10.0, 20.0);
    state.acquire_lock("page");
    state.advance_time(50);
    state.record_navigation(url::Url::parse("https://example.test").unwrap());
    assert_eq!(
        state.normalized_values.get("total").map(String::as_str),
        Some("42.00")
    );
    assert_eq!(state.history[0].observed_at_millis, 50);
    assert_eq!(state.autocomplete, vec!["42.01", "42.02"]);
    assert!(state.is_locked("page"));
    assert!(state.release_lock("page"));
}
