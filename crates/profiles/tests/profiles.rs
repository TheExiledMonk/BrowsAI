use browsai_profiles::ProfileManager;

#[test]
fn profiles_have_isolated_identity() {
    let mut manager = ProfileManager::default();
    let first = manager.create("work");
    let second = manager.create("personal");
    assert_ne!(first, second);
    assert_eq!(manager.list().count(), 2);
    assert_eq!(manager.get(&first).unwrap().name, "work");
}

#[test]
fn browser_identity_and_history_are_profile_scoped_and_bounded() {
    let mut manager = ProfileManager::default();
    let profile = manager.create("work");
    let other = manager.create("personal");
    let mut config = manager.get(&profile).unwrap().config.clone();
    config.user_agent = Some("BrowsAI-Test/1.0".into());
    config.locale = "de-DE".into();
    assert!(manager.update_config(&profile, config));
    assert!(manager.record_history(
        &profile,
        browsai_profiles::HistoryEntry {
            url: "https://a.test".into(),
            title: None,
            visited_at_unix_seconds: 1
        },
        1
    ));
    assert!(manager.record_history(
        &profile,
        browsai_profiles::HistoryEntry {
            url: "https://b.test".into(),
            title: None,
            visited_at_unix_seconds: 2
        },
        1
    ));
    assert_eq!(manager.get(&profile).unwrap().config.history.len(), 1);
    assert_eq!(
        manager.get(&profile).unwrap().config.user_agent.as_deref(),
        Some("BrowsAI-Test/1.0")
    );
    assert!(manager.get(&other).unwrap().config.history.is_empty());
}

#[test]
fn profile_selection_and_persistence_preserve_generations() {
    let mut manager = ProfileManager::default();
    let first = manager.create("work");
    let second = manager.create("personal");
    assert_eq!(manager.selected().unwrap().id, first);
    assert!(manager.select(&second));
    let mut config = manager.get(&second).unwrap().config.clone();
    config.locale = "fr-FR".into();
    assert!(manager.update_config(&second, config));
    let restored = ProfileManager::from_json(&manager.to_json().unwrap()).unwrap();
    assert_eq!(restored.selected().unwrap().id, second);
    assert_eq!(restored.get(&second).unwrap().generation, 1);
    assert_eq!(restored.get(&second).unwrap().config.locale, "fr-FR");
    assert!(manager.delete(&second));
    assert_eq!(manager.selected().unwrap().id, first);
}

#[test]
fn profile_limits_and_schema_versioning_bound_persistent_state() {
    let mut manager = ProfileManager::default();
    manager.set_limits(browsai_profiles::ProfileLimits {
        max_profiles: 1,
        max_history_entries: 1,
        max_autocomplete_entries: 1,
        max_extensions: 1,
    });
    assert!(manager.try_create("work").is_some());
    assert!(manager.try_create("personal").is_none());
    let profile = manager.selected().unwrap().id.clone();
    assert!(manager.update_config(
        &profile,
        browsai_profiles::ProfileConfig {
            extensions: vec!["a".into(), "b".into()],
            ..Default::default()
        }
    ));
    assert_eq!(manager.get(&profile).unwrap().config.extensions.len(), 1);
    let encoded = manager.to_json().unwrap();
    assert!(encoded.contains("schema_version"));
    assert_eq!(
        ProfileManager::from_json(&encoded)
            .unwrap()
            .limits()
            .max_profiles,
        1
    );
}
