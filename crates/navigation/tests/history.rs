use browsai_navigation::{NavigationState, NavigationStatus};
use url::Url;

#[test]
fn navigation_tracks_history_and_same_document_changes() {
    let mut state = NavigationState::new(Url::parse("https://example.test/").unwrap());
    state.start(Url::parse("https://example.test/#one").unwrap());
    assert_eq!(state.history.len(), 1);
    state.complete();
    state.start(Url::parse("https://other.test/").unwrap());
    assert_eq!(state.history.len(), 2);
    assert!(state.can_back());
    assert_eq!(state.back().unwrap().as_str(), "https://example.test/#one");
    assert!(matches!(state.status, NavigationStatus::Loading));
}

#[test]
fn reload_reuses_current_history_entry_and_origin_transitions_are_explicit() {
    let mut state = NavigationState::new(Url::parse("https://example.test/app").unwrap());
    let generation = state.generation;
    let reloaded = state.reload();
    assert_eq!(reloaded.as_str(), "https://example.test/app");
    assert_eq!(state.history.len(), 1);
    assert_eq!(state.generation, generation + 1);
    assert!(matches!(state.status, NavigationStatus::Loading));
    assert!(state.is_cross_origin(&Url::parse("https://other.test/app").unwrap()));
    assert!(!state.is_cross_origin(&Url::parse("https://example.test/other").unwrap()));
    state.stop();
    assert!(matches!(state.status, NavigationStatus::Stopped));
}

#[test]
fn navigating_to_the_current_url_does_not_duplicate_history() {
    let url = Url::parse("https://example.test/app?view=all#top").unwrap();
    let mut state = NavigationState::new(url.clone());
    state.start(url);
    assert_eq!(state.history.len(), 1);
    assert_eq!(state.index, 0);
}
