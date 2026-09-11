use browsai_password_vault::PasswordManager;

#[test]
fn login_detection_requires_username_and_password_fields() {
    let form = PasswordManager::detect_login_form(
        "https://example.test",
        &[("email", "text"), ("pass", "password")],
    )
    .unwrap();
    assert_eq!(form.origin, "https://example.test");
    assert_eq!(form.username_field, "email");
    assert_eq!(form.password_field, "pass");
    assert!(
        PasswordManager::detect_login_form("https://example.test", &[("email", "text")]).is_none()
    );
}
