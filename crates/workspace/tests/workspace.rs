use browsai_profiles::ProfileManager;
use browsai_tabs::TabId;
use browsai_workspace::{Workspace, WorkspaceError};
use url::Url;

#[test]
fn workspace_owns_windows_tabs_and_locks() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut workspace = Workspace::new(profile.clone());
    let window = workspace.create_window(1280, 720);
    let tab = workspace
        .tabs
        .open(profile, Url::parse("about:blank").unwrap());
    assert!(workspace.attach_tab(&window, tab));
    assert!(workspace.acquire_lock("agent:primary"));
    assert!(!workspace.acquire_lock("agent:primary"));
    assert!(workspace.release_lock("agent:primary"));
    assert!(workspace.grant_permission("downloads:write"));
    assert!(workspace.has_permission("downloads:write"));
    assert!(workspace.revoke_permission("downloads:write"));
}

#[test]
fn workspace_rejects_missing_and_cross_profile_tabs() {
    let mut profiles = ProfileManager::default();
    let work = profiles.create("work");
    let other = profiles.create("other");
    let mut workspace = Workspace::new(work.clone());
    let window = workspace.create_window(800, 600);
    let valid = workspace
        .tabs
        .open(work, Url::parse("about:blank").unwrap());
    let foreign = workspace
        .tabs
        .open(other, Url::parse("about:blank").unwrap());
    assert!(workspace.attach_tab(&window, valid));
    assert!(!workspace.attach_tab(&window, foreign));
    assert!(!workspace.attach_tab(&window, TabId::new()));
}

#[test]
fn workspace_rejects_duplicate_tabs_and_invalid_checkpoint_membership() {
    let profile = ProfileManager::default().create("work");
    let mut workspace = Workspace::new(profile.clone());
    let first_window = workspace.create_window(800, 600);
    let second_window = workspace.create_window(800, 600);
    let tab = workspace
        .tabs
        .open(profile, Url::parse("about:blank").unwrap());
    assert!(workspace.attach_tab(&first_window, tab.clone()));
    assert!(!workspace.attach_tab(&second_window, tab));

    let mut checkpoint = workspace.checkpoint();
    let duplicate = checkpoint.windows.get(&first_window).unwrap().tabs[0].clone();
    checkpoint
        .windows
        .get_mut(&second_window)
        .unwrap()
        .tabs
        .push(duplicate);
    assert!(matches!(
        Workspace::from_checkpoint(checkpoint, &workspace.profile),
        Err(WorkspaceError::InvalidCheckpoint)
    ));
}

#[test]
fn workspace_checkpoint_round_trips_and_rejects_wrong_profile() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let other = profiles.create("other");
    let mut workspace = Workspace::new(profile.clone());
    let window = workspace.create_window(800, 600);
    let tab = workspace
        .tabs
        .open(profile.clone(), Url::parse("https://example.test").unwrap());
    assert!(workspace.attach_tab(&window, tab));
    let checkpoint = workspace.checkpoint();
    let encoded = serde_json::to_string(&checkpoint).unwrap();
    let decoded: browsai_workspace::WorkspaceCheckpoint = serde_json::from_str(&encoded).unwrap();
    assert!(browsai_workspace::Workspace::from_checkpoint(decoded.clone(), &profile).is_ok());
    assert!(matches!(
        browsai_workspace::Workspace::from_checkpoint(decoded, &other),
        Err(browsai_workspace::WorkspaceError::ProfileMismatch)
    ));
}

#[test]
fn workspace_hands_off_window_focus_when_windows_change() {
    let profile = ProfileManager::default().create("work");
    let mut workspace = Workspace::new(profile);
    let first = workspace.create_window(800, 600);
    let second = workspace.create_window(1024, 768);
    assert!(workspace.focus_window(&second));
    assert!(!workspace.windows.get(&first).unwrap().focused);
    assert!(workspace.windows.get(&second).unwrap().focused);
    assert!(workspace.close_window(&second).is_some());
    assert!(workspace.windows.get(&first).unwrap().focused);
    assert!(!workspace.focus_window(&second));
}
