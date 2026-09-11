use browsai_payment_vault::PaymentVault;
use browsai_profiles::ProfileManager;
use browsai_transactions::{PolicyDecision, TransactionPolicy};

#[test]
fn payment_vault_requires_confirmation_and_origin_binding() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut vault = PaymentVault::default();
    let id = vault.add_card(profile.clone(), "https://shop.example", b"4111111111111111");
    assert_eq!(
        vault
            .classify_purchase(&TransactionPolicy::default())
            .decision,
        PolicyDecision::RequireConfirmation
    );
    assert!(vault
        .authorize(&id, profile, "https://other.example")
        .is_err());
}

#[test]
fn payment_release_requires_confirmation_and_audits_non_sensitive_steps() {
    let profile = ProfileManager::default().create("work");
    let mut vault = PaymentVault::default();
    let id = vault.add_card(profile.clone(), "https://shop.example", b"4111111111111111");
    assert!(vault
        .authorize(&id, profile.clone(), "https://shop.example")
        .is_err());
    let capability = vault
        .authorize_with_confirmation(&id, profile.clone(), "https://shop.example", true)
        .unwrap();
    vault
        .with_card_number(&capability, &profile, "https://shop.example", |number| {
            number.len()
        })
        .unwrap();
    let events: Vec<_> = vault.drain_events().collect();
    assert_eq!(events[0].action, "card_added");
    assert_eq!(events[1].action, "payment_authorized");
    assert_eq!(events[2].action, "payment_released");
}
