use browsai_password_vault::{LoginForm, PasswordManager};
use browsai_profiles::ProfileManager;

#[test]
fn password_manager_plans_autofill_without_exposing_password() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut manager = PasswordManager::default();
    let id = manager.save(profile.clone(), "https://example.test", "alice", b"secret");
    let plan = manager
        .plan_autofill(
            &profile,
            &LoginForm {
                origin: "https://example.test".into(),
                username_field: "email".into(),
                password_field: "password".into(),
            },
        )
        .unwrap();
    assert_eq!(plan.0, id);
    assert_eq!(plan.1, "alice");
}
