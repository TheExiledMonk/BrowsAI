use browsai_profiles::ProfileManager;
use browsai_tabs::TabManager;
use url::Url;

#[test]
fn tab_manager_tracks_active_and_background_tabs() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut tabs = TabManager::default();
    let first = tabs.open(profile.clone(), Url::parse("about:blank").unwrap());
    let second = tabs.open(profile.clone(), Url::parse("https://example.test").unwrap());
    assert_eq!(tabs.active().unwrap().id, first);
    assert!(tabs.activate(&second));
    assert!(tabs.get(&second).unwrap().active);
    assert!(tabs.get(&first).unwrap().background);
}

#[test]
fn tab_navigation_helpers_update_history_and_generation() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut tabs = TabManager::default();
    let tab = tabs.open(profile, Url::parse("https://example.test/one").unwrap());
    assert_eq!(
        tabs.navigate(&tab, Url::parse("https://example.test/two").unwrap()),
        Some(Url::parse("https://example.test/two").unwrap())
    );
    assert_eq!(tabs.get(&tab).unwrap().generation, 1);
    assert_eq!(
        tabs.back(&tab).unwrap().as_str(),
        "https://example.test/one"
    );
    assert_eq!(tabs.get(&tab).unwrap().generation, 2);
    assert_eq!(
        tabs.forward(&tab).unwrap().as_str(),
        "https://example.test/two"
    );
    assert_eq!(tabs.get(&tab).unwrap().generation, 3);
}

#[test]
fn tabs_round_trip_and_scope_by_profile_with_one_active_tab() {
    let mut profiles = ProfileManager::default();
    let work = profiles.create("work");
    let personal = profiles.create("personal");
    let mut tabs = TabManager::default();
    tabs.open(work.clone(), Url::parse("about:blank").unwrap());
    let personal_tab = tabs.open(
        personal.clone(),
        Url::parse("https://example.test").unwrap(),
    );
    assert!(tabs.activate(&personal_tab));
    let encoded = tabs.to_json().unwrap();
    let restored = TabManager::from_json(&encoded).unwrap();
    assert_eq!(restored.for_profile(&work).count(), 1);
    assert_eq!(restored.for_profile(&personal).count(), 1);
    assert_eq!(restored.active().unwrap().id, personal_tab);
    assert_eq!(restored.list().filter(|tab| tab.active).count(), 1);
}

#[test]
fn tab_snapshots_and_background_policy_follow_activation_and_restore() {
    let profile = ProfileManager::default().create("work");
    let mut tabs = TabManager::default();
    let first = tabs.open(profile.clone(), Url::parse("about:blank").unwrap());
    let second = tabs.open(profile.clone(), Url::parse("https://example.test").unwrap());
    assert_eq!(
        tabs.snapshot(&first).unwrap().execution.max_tasks_per_tick,
        1_000
    );
    assert_eq!(
        tabs.snapshot(&second)
            .unwrap()
            .execution
            .minimum_timer_interval_millis,
        1_000
    );

    tabs.activate(&second);
    let snapshot = tabs.snapshot(&second).unwrap();
    assert!(snapshot.active && !snapshot.background);
    assert_eq!(snapshot.execution.minimum_timer_interval_millis, 16);

    let restored = TabManager::from_json(&tabs.to_json().unwrap()).unwrap();
    assert_eq!(
        restored
            .snapshot(&first)
            .unwrap()
            .execution
            .max_tasks_per_tick,
        50
    );
    assert_eq!(
        restored
            .snapshot(&second)
            .unwrap()
            .execution
            .max_tasks_per_tick,
        1_000
    );
    assert_eq!(restored.agent_scope(&profile).len(), 2);
}
