use browsai_permissions::{Permission, PermissionDecision, PermissionManager};
use browsai_profiles::ProfileManager;
use std::time::Duration;

#[test]
fn permissions_are_origin_and_profile_scoped() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut permissions = PermissionManager::default();
    assert_eq!(
        permissions.check(&profile, "https://example.test", Permission::ClipboardRead),
        PermissionDecision::Prompt
    );
    permissions.set(
        &profile,
        "https://example.test",
        Permission::ClipboardRead,
        true,
    );
    assert_eq!(
        permissions.check(&profile, "https://example.test", Permission::ClipboardRead),
        PermissionDecision::Granted
    );
    assert_eq!(
        permissions.check(&profile, "https://other.test", Permission::ClipboardRead),
        PermissionDecision::Prompt
    );
}

#[test]
fn requests_support_scoped_expiring_grants_and_revocation() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut permissions = PermissionManager::default();
    let request = browsai_permissions::PermissionRequest {
        origin: "https://example.test".into(),
        permission: Permission::Downloads,
        reason: "save a user-requested file".into(),
    };
    assert_eq!(
        permissions.request(&profile, &request),
        PermissionDecision::Prompt
    );
    let grant = permissions.grant_for(&profile, &request, Some(Duration::from_secs(60)));
    assert!(grant.expires_at_unix_seconds.is_some());
    assert_eq!(
        permissions.request(&profile, &request),
        PermissionDecision::Granted
    );
    permissions.revoke(&profile, &request.origin, request.permission);
    assert_eq!(
        permissions.request(&profile, &request),
        PermissionDecision::Prompt
    );
}

#[test]
fn permission_grants_round_trip_with_profile_scope() {
    let profile = ProfileManager::default().create("work");
    let mut permissions = PermissionManager::default();
    permissions.set(
        &profile,
        "https://example.test",
        Permission::ClipboardRead,
        true,
    );
    let restored = PermissionManager::from_json(&permissions.to_json().unwrap()).unwrap();
    assert_eq!(
        restored.check(&profile, "https://example.test", Permission::ClipboardRead),
        PermissionDecision::Granted
    );
    assert_eq!(
        restored.check(&profile, "https://other.test", Permission::ClipboardRead),
        PermissionDecision::Prompt
    );
}

#[test]
fn denied_decisions_persist_until_explicit_revocation() {
    let profile = ProfileManager::default().create("work");
    let mut permissions = PermissionManager::default();
    permissions.set(&profile, "https://example.test", Permission::Camera, false);
    assert_eq!(
        permissions.check(&profile, "https://example.test", Permission::Camera),
        PermissionDecision::Denied
    );
    let restored = PermissionManager::from_json(&permissions.to_json().unwrap()).unwrap();
    assert_eq!(
        restored.check(&profile, "https://example.test", Permission::Camera),
        PermissionDecision::Denied
    );
    permissions.revoke(&profile, "https://example.test", Permission::Camera);
    assert_eq!(
        permissions.check(&profile, "https://example.test", Permission::Camera),
        PermissionDecision::Prompt
    );
}

#[test]
fn purging_expired_grants_does_not_leave_a_stale_granted_decision() {
    let profile = ProfileManager::default().create("work");
    let mut permissions = PermissionManager::default();
    let request = browsai_permissions::PermissionRequest {
        origin: "https://example.test".into(),
        permission: Permission::Notifications,
        reason: "notify".into(),
    };
    permissions.grant_for(&profile, &request, Some(Duration::ZERO));
    assert!(permissions.purge_expired() >= 1);
    assert_eq!(
        permissions.check(&profile, &request.origin, request.permission),
        PermissionDecision::Prompt
    );
}

#[test]
fn extended_device_capabilities_are_typed_and_persisted() {
    let profile = ProfileManager::default().create("work");
    let mut permissions = PermissionManager::default();
    for permission in [
        Permission::Certificates,
        Permission::Proxy,
        Permission::Device,
    ] {
        permissions.set(&profile, "https://example.test", permission, true);
    }
    let restored = PermissionManager::from_json(&permissions.to_json().unwrap()).unwrap();
    for permission in [
        Permission::Certificates,
        Permission::Proxy,
        Permission::Device,
    ] {
        assert_eq!(
            restored.check(&profile, "https://example.test", permission),
            PermissionDecision::Granted
        );
        assert_eq!(
            restored.check(&profile, "https://other.test", permission),
            PermissionDecision::Prompt
        );
    }
}

#[test]
fn permission_requests_decisions_expiry_and_revocation_are_observable() {
    let profile = ProfileManager::default().create("work");
    let mut permissions = PermissionManager::default();
    let request = browsai_permissions::PermissionRequest {
        origin: "https://example.test".into(),
        permission: Permission::Camera,
        reason: "video call".into(),
    };
    assert_eq!(
        permissions.request(&profile, &request),
        PermissionDecision::Prompt
    );
    permissions.grant_for(&profile, &request, Some(Duration::ZERO));
    permissions.purge_expired();
    permissions.revoke(&profile, &request.origin, request.permission);
    let events: Vec<_> = permissions.drain_events().collect();
    assert!(events.iter().any(|event| matches!(
        event,
        browsai_permissions::PermissionEvent::Requested { .. }
    )));
    assert!(events
        .iter()
        .any(|event| matches!(event, browsai_permissions::PermissionEvent::Expired { .. })));
    assert!(events
        .iter()
        .any(|event| matches!(event, browsai_permissions::PermissionEvent::Revoked { .. })));
}
