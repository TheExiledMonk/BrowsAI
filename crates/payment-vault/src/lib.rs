use browsai_profiles::ProfileId;
use browsai_secret_store::{SecretBroker, SecretCapability, SecretError};
use browsai_transactions::{
    Consequence, Reversibility, TransactionClassification, TransactionPolicy,
};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PaymentId(Uuid);

#[derive(Clone, Debug)]
pub struct PaymentInstrument {
    pub id: PaymentId,
    pub profile: ProfileId,
    pub merchant_origin: String,
    number_handle: browsai_secret_store::SecretHandle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentAuditEvent {
    pub payment_id: PaymentId,
    pub action: &'static str,
}

#[derive(Default)]
pub struct PaymentVault {
    broker: SecretBroker,
    instruments: BTreeMap<PaymentId, PaymentInstrument>,
    events: Vec<PaymentAuditEvent>,
}

impl PaymentVault {
    pub fn add_card(
        &mut self,
        profile: ProfileId,
        merchant_origin: impl Into<String>,
        card_number: impl AsRef<[u8]>,
    ) -> PaymentId {
        let merchant_origin = merchant_origin.into();
        let id = PaymentId(Uuid::new_v4());
        let handle = self
            .broker
            .store(profile.clone(), merchant_origin.clone(), card_number);
        self.instruments.insert(
            id.clone(),
            PaymentInstrument {
                id: id.clone(),
                profile,
                merchant_origin,
                number_handle: handle,
            },
        );
        self.events.push(PaymentAuditEvent {
            payment_id: id.clone(),
            action: "card_added",
        });
        id
    }
    pub fn classify_purchase(&self, policy: &TransactionPolicy) -> TransactionClassification {
        policy.classify(Consequence::Purchase, Reversibility::RemoteIrreversible)
    }
    pub fn authorize(
        &mut self,
        id: &PaymentId,
        profile: ProfileId,
        merchant_origin: &str,
    ) -> Result<SecretCapability, SecretError> {
        self.authorize_with_confirmation(id, profile, merchant_origin, false)
    }

    pub fn authorize_with_confirmation(
        &mut self,
        id: &PaymentId,
        profile: ProfileId,
        merchant_origin: &str,
        confirmed: bool,
    ) -> Result<SecretCapability, SecretError> {
        let instrument = self.instruments.get(id).ok_or(SecretError::UnknownHandle)?;
        if instrument.profile != profile {
            return Err(SecretError::ProfileMismatch);
        }
        if instrument.merchant_origin != merchant_origin {
            return Err(SecretError::OriginMismatch);
        }
        if !confirmed {
            return Err(SecretError::InvalidCapability);
        }
        let capability = self.broker.grant(
            &instrument.number_handle,
            profile,
            merchant_origin,
            "payment",
            true,
        )?;
        self.events.push(PaymentAuditEvent {
            payment_id: id.clone(),
            action: "payment_authorized",
        });
        Ok(capability)
    }
    pub fn with_card_number<T>(
        &mut self,
        capability: &SecretCapability,
        profile: &ProfileId,
        origin: &str,
        use_number: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, SecretError> {
        let result = self
            .broker
            .with_secret(capability, profile, origin, use_number);
        if result.is_ok() {
            if let Some((payment_id, _)) = self
                .instruments
                .iter()
                .find(|(_, instrument)| instrument.number_handle == capability.handle)
            {
                self.events.push(PaymentAuditEvent {
                    payment_id: payment_id.clone(),
                    action: "payment_released",
                });
            }
        }
        result
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = PaymentAuditEvent> + '_ {
        self.events.drain(..)
    }
}
