use browsai_ipc::{IpcChannel, IpcEnvelope, IpcMessage, IpcRouter, ProcessKind};
use uuid::Uuid;

fn message(source: ProcessKind, destination: ProcessKind) -> IpcEnvelope {
    IpcEnvelope {
        protocol_version: 1,
        sequence: 0,
        request_id: Uuid::new_v4(),
        source,
        destination,
        workspace: "w".into(),
        profile: "p".into(),
        origin: None,
        capability: None,
        payload: IpcMessage::Handshake {
            protocol_version: 1,
        },
    }
}

#[test]
fn ipc_router_rejects_unapproved_process_edges() {
    let mut router = IpcRouter::new(1);
    router.allow(ProcessKind::Renderer, ProcessKind::BrowserBroker);
    assert!(router
        .validate(&message(ProcessKind::Renderer, ProcessKind::BrowserBroker))
        .is_ok());
    assert!(router
        .validate(&message(
            ProcessKind::Renderer,
            ProcessKind::CredentialBroker
        ))
        .is_err());
}

#[test]
fn ipc_channel_cancels_requests_and_reconnects_cleanly() {
    let mut channel = IpcChannel::bounded(2);
    let first = message(ProcessKind::Renderer, ProcessKind::BrowserBroker);
    let request_id = first.request_id;
    channel.send(first).unwrap();
    let mut second = message(ProcessKind::Renderer, ProcessKind::BrowserBroker);
    second.request_id = uuid::Uuid::new_v4();
    channel.send(second).unwrap();
    assert_eq!(channel.cancel(request_id), 1);
    assert_eq!(channel.receive().unwrap().sequence, 1);
    channel.reconnect();
    let fresh = message(ProcessKind::Renderer, ProcessKind::BrowserBroker);
    channel.send(fresh).unwrap();
    assert_eq!(channel.receive().unwrap().sequence, 0);
}

#[test]
fn ipc_router_rejects_handshake_payload_version_mismatch() {
    let mut router = IpcRouter::new(1);
    router.allow(ProcessKind::Renderer, ProcessKind::BrowserBroker);
    let mut envelope = message(ProcessKind::Renderer, ProcessKind::BrowserBroker);
    envelope.payload = IpcMessage::Handshake {
        protocol_version: 2,
    };
    assert_eq!(
        router.validate(&envelope),
        Err(browsai_ipc::IpcError::VersionMismatch)
    );
}

#[test]
fn ipc_router_rejects_nested_raw_secret_fields_but_keeps_opaque_capabilities() {
    let mut router = IpcRouter::new(1);
    router.allow(ProcessKind::BrowserBroker, ProcessKind::CredentialBroker);
    let mut envelope = message(ProcessKind::BrowserBroker, ProcessKind::CredentialBroker);
    envelope.capability = Some("capability-reference-only".into());
    envelope.payload = IpcMessage::Request {
        method: "credential-release".into(),
        body: serde_json::json!({"request": {"authToken": "plaintext"}}),
    };
    assert_eq!(
        router.validate(&envelope),
        Err(browsai_ipc::IpcError::RawSecretPayload)
    );

    envelope.payload = IpcMessage::Request {
        method: "credential-release".into(),
        body: serde_json::json!({"capability": "opaque-reference", "purpose": "login"}),
    };
    assert!(router.validate(&envelope).is_ok());
}
