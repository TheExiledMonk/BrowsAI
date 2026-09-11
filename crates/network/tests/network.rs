use browsai_network::{
    NetworkAuditEvent, NetworkError, NetworkEvent, NetworkRequest, NetworkResponse, NetworkStack,
    NormalizedNetworkError, WebSocketFrame,
};
use browsai_profiles::ProfileManager;
use std::collections::BTreeMap;
use url::Url;
use uuid::Uuid;

#[test]
fn network_stack_records_request_lifecycle() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let url = Url::parse("https://example.test/api").unwrap();
    let mut stack = NetworkStack::default();
    stack.route(
        "GET",
        &url,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 200,
            headers: BTreeMap::new(),
            body: b"ok".to_vec(),
        },
    );
    let request_id = Uuid::new_v4();
    let response = stack
        .fetch(NetworkRequest {
            id: request_id,
            profile,
            top_level_site: "https://example.test".into(),
            url,
            method: "GET".into(),
            headers: BTreeMap::new(),
            body: None,
            initiator: Some("document".into()),
        })
        .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.request_id, request_id);
    assert_eq!(
        stack
            .events()
            .iter()
            .filter(|event| matches!(
                event,
                NetworkEvent::RequestStarted { .. } | NetworkEvent::RequestCompleted { .. }
            ))
            .count(),
        2
    );
}

#[test]
fn network_observation_preserves_initiator_response_and_mutation_correlation() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let url = Url::parse("https://example.test/data").unwrap();
    let mut network = NetworkStack::default();
    network.route(
        "POST",
        &url,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 204,
            headers: BTreeMap::new(),
            body: vec![],
        },
    );
    let request_id = Uuid::new_v4();
    network
        .fetch(NetworkRequest {
            id: request_id,
            profile,
            top_level_site: "https://example.test".into(),
            url,
            method: "POST".into(),
            headers: BTreeMap::new(),
            body: None,
            initiator: Some("form:save".into()),
        })
        .unwrap();
    assert!(network.events().iter().any(|event| matches!(
        event,
        NetworkEvent::RequestInitiator { request_id: id, initiator }
            if *id == request_id && initiator == "form:save"
    )));
    assert!(network.events().iter().any(|event| matches!(
        event,
        NetworkEvent::ResponseReceived { request_id: id, status: 204, .. }
            if *id == request_id
    )));
    assert!(network.events().iter().any(|event| matches!(
        event,
        NetworkEvent::Timing { request_id: id, elapsed_millis: 0 }
            if *id == request_id
    )));
    assert!(network.events().iter().any(|event| matches!(
        event,
        NetworkEvent::Mutation { request_id: id, kind }
            if *id == request_id && kind == "cacheInvalidated"
    )));
    assert_eq!(network.drain_events().count(), 6);
    assert!(network.events().is_empty());
}

#[test]
fn network_limits_and_metrics_are_enforced() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let url = Url::parse("https://example.test/api").unwrap();
    let mut stack = NetworkStack::default();
    stack.limits.max_requests = 1;
    stack.route(
        "GET",
        &url,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 200,
            headers: BTreeMap::new(),
            body: b"ok".to_vec(),
        },
    );
    let request = || NetworkRequest {
        id: Uuid::new_v4(),
        profile: profile.clone(),
        top_level_site: "https://example.test".into(),
        url: url.clone(),
        method: "GET".into(),
        headers: BTreeMap::new(),
        body: None,
        initiator: None,
    };
    assert!(stack.fetch(request()).is_ok());
    assert_eq!(stack.fetch(request()), Err(NetworkError::RequestLimit));
    assert_eq!(stack.metrics.requests_completed, 1);
    assert_eq!(stack.metrics.bytes_received, 2);
    assert!(matches!(
        stack.audit_events().first(),
        Some(NetworkAuditEvent::Completed {
            status: 200,
            bytes: 2,
            ..
        })
    ));
    assert!(matches!(
        stack.audit_events().last(),
        Some(NetworkAuditEvent::Failed {
            error: NormalizedNetworkError::RequestLimit,
            ..
        })
    ));
}

#[test]
fn normalized_failures_are_audited_with_request_identity() {
    let profile = ProfileManager::default().create("work");
    let mut stack = NetworkStack::default();
    let request_id = Uuid::new_v4();
    let error = stack.fetch(NetworkRequest {
        id: request_id,
        profile,
        top_level_site: "https://example.test".into(),
        url: Url::parse("https://example.test/missing").unwrap(),
        method: "GET".into(),
        headers: BTreeMap::new(),
        body: None,
        initiator: None,
    });
    assert_eq!(error, Err(NetworkError::NoRoute));
    assert!(matches!(
        stack.drain_audit_events().next(),
        Some(NetworkAuditEvent::Failed { request_id: id, error: NormalizedNetworkError::NoRoute })
            if id == request_id
    ));
}

#[test]
fn redirects_follow_location_with_a_bound() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let first = Url::parse("https://example.test/start").unwrap();
    let final_url = Url::parse("https://example.test/final").unwrap();
    let mut stack = NetworkStack::default();
    stack.route(
        "GET",
        &first,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 302,
            headers: [("location".into(), "/final".into())].into_iter().collect(),
            body: vec![],
        },
    );
    stack.route(
        "GET",
        &final_url,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 200,
            headers: BTreeMap::new(),
            body: b"ok".to_vec(),
        },
    );
    let response = stack
        .fetch(NetworkRequest {
            id: Uuid::new_v4(),
            profile,
            top_level_site: "https://example.test".into(),
            url: first,
            method: "GET".into(),
            headers: BTreeMap::new(),
            body: None,
            initiator: None,
        })
        .unwrap();
    assert_eq!(response.status, 200);
}

#[test]
fn get_responses_are_cached_by_profile_and_top_level_site() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let url = Url::parse("https://example.test/api").unwrap();
    let mut stack = NetworkStack::default();
    stack.limits.max_requests = 2;
    stack.route(
        "GET",
        &url,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 200,
            headers: BTreeMap::new(),
            body: b"cached".to_vec(),
        },
    );
    let request = |top_level_site: &str| NetworkRequest {
        id: Uuid::new_v4(),
        profile: profile.clone(),
        top_level_site: top_level_site.into(),
        url: url.clone(),
        method: "GET".into(),
        headers: BTreeMap::new(),
        body: None,
        initiator: None,
    };
    assert_eq!(
        stack.fetch(request("https://example.test")).unwrap().body,
        b"cached"
    );
    assert_eq!(
        stack.fetch(request("https://example.test")).unwrap().body,
        b"cached"
    );
    assert_eq!(stack.cache.len(), 1);
    assert!(stack.fetch(request("https://other.test")).is_err());
}

#[test]
fn websocket_connections_are_gated_and_preserve_frame_order() {
    let url = Url::parse("wss://example.test/events").unwrap();
    let mut stack = NetworkStack::default();
    stack.route_websocket(&url, vec![WebSocketFrame::Text("ready".into())]);
    assert_eq!(
        stack.connect_websocket(&url),
        Err(NetworkError::WebSocketDisabled)
    );
    stack.config.websocket_enabled = true;
    let mut connection = stack.connect_websocket(&url).unwrap();
    connection
        .send(WebSocketFrame::Text("subscribe".into()))
        .unwrap();
    assert_eq!(
        connection.receive(),
        Some(WebSocketFrame::Text("ready".into()))
    );
    assert_eq!(
        connection.sent_frames(),
        &[WebSocketFrame::Text("subscribe".into())]
    );
    connection.send(WebSocketFrame::Close).unwrap();
    assert!(connection.is_closed());
}

#[test]
fn enabled_service_workers_intercept_only_controlled_network_requests() {
    let profile = ProfileManager::default().create("work");
    let scope = Url::parse("https://example.test/app/").unwrap();
    let controlled = Url::parse("https://example.test/app/data").unwrap();
    let mut stack = NetworkStack::default();
    stack.config.service_workers_enabled = true;
    stack.register_service_worker(scope, Url::parse("https://example.test/sw.js").unwrap());
    stack.intercept_service_worker(
        &controlled,
        browsai_service_worker::FetchResponse {
            status: 200,
            body: b"worker-response".to_vec(),
        },
    );
    let response = stack
        .fetch(NetworkRequest {
            id: Uuid::new_v4(),
            profile,
            top_level_site: "https://example.test".into(),
            url: controlled,
            method: "GET".into(),
            headers: BTreeMap::new(),
            body: None,
            initiator: Some("service-worker-test".into()),
        })
        .unwrap();
    assert_eq!(response.body, b"worker-response");
    assert_eq!(stack.metrics.requests_completed, 1);
}

#[test]
fn network_facade_can_unregister_service_worker_control() {
    let profile = ProfileManager::default().create("work");
    let scope = Url::parse("https://example.test/app/").unwrap();
    let controlled = Url::parse("https://example.test/app/data").unwrap();
    let mut stack = NetworkStack::default();
    stack.config.service_workers_enabled = true;
    stack.register_service_worker(
        scope.clone(),
        Url::parse("https://example.test/sw.js").unwrap(),
    );
    stack.intercept_service_worker(
        &controlled,
        browsai_service_worker::FetchResponse {
            status: 200,
            body: b"worker-response".to_vec(),
        },
    );
    assert!(stack.unregister_service_worker(&scope));
    let result = stack.fetch(NetworkRequest {
        id: Uuid::new_v4(),
        profile,
        top_level_site: "https://example.test".into(),
        url: controlled,
        method: "GET".into(),
        headers: BTreeMap::new(),
        body: None,
        initiator: Some("service-worker-test".into()),
    });
    assert_eq!(result, Err(NetworkError::NoRoute));
}

#[test]
fn missing_routes_record_request_failure_with_request_identity() {
    let profile = ProfileManager::default().create("work");
    let request_id = Uuid::new_v4();
    let mut stack = NetworkStack::default();
    let result = stack.fetch(NetworkRequest {
        id: request_id,
        profile,
        top_level_site: "https://example.test".into(),
        url: Url::parse("https://example.test/missing").unwrap(),
        method: "GET".into(),
        headers: BTreeMap::new(),
        body: None,
        initiator: None,
    });
    assert_eq!(result, Err(NetworkError::NoRoute));
    assert_eq!(stack.metrics.requests_started, 1);
    assert_eq!(stack.metrics.requests_failed, 1);
    assert!(matches!(
        stack.events().last(),
        Some(browsai_network::NetworkEvent::RequestFailed { request_id: id, .. }) if *id == request_id
    ));
}

#[test]
fn routed_set_cookie_is_sent_on_the_next_partitioned_request() {
    let profile = ProfileManager::default().create("work");
    let login = Url::parse("https://example.test/login").unwrap();
    let data = Url::parse("https://example.test/data").unwrap();
    let mut stack = NetworkStack::default();
    stack.route(
        "POST",
        &login,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 204,
            headers: [("Set-Cookie".into(), "session=abc; Path=/".into())]
                .into_iter()
                .collect(),
            body: vec![],
        },
    );
    stack.route(
        "GET",
        &data,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 200,
            headers: BTreeMap::new(),
            body: b"ok".to_vec(),
        },
    );
    let request = |url: Url, method: &str| NetworkRequest {
        id: Uuid::new_v4(),
        profile: profile.clone(),
        top_level_site: "https://example.test".into(),
        url,
        method: method.into(),
        headers: BTreeMap::new(),
        body: None,
        initiator: None,
    };
    stack.fetch(request(login, "POST")).unwrap();
    stack.fetch(request(data, "GET")).unwrap();
    assert_eq!(
        stack
            .events()
            .iter()
            .filter(|event| matches!(
                event,
                NetworkEvent::RequestStarted { .. } | NetworkEvent::RequestCompleted { .. }
            ))
            .count(),
        4,
        "both requests should complete through the normal lifecycle"
    );
    assert_eq!(
        stack.cookies.header_for_at(
            &profile,
            "https://example.test",
            &Url::parse("https://example.test/data").unwrap(),
            0,
        ),
        "session=abc"
    );
    assert!(matches!(
        stack.audit_events().iter().find(|event| matches!(event, NetworkAuditEvent::CookieStored { .. })),
        Some(NetworkAuditEvent::CookieStored { domain, .. }) if domain == "example.test"
    ));
    let snapshot = stack.export_cookie_snapshot().unwrap();
    let mut restored = NetworkStack::default();
    restored.restore_cookie_snapshot(&snapshot).unwrap();
    assert_eq!(
        restored.cookies.header_for_at(
            &profile,
            "https://example.test",
            &Url::parse("https://example.test/data").unwrap(),
            0,
        ),
        "session=abc"
    );
}

#[test]
fn configured_proxy_routes_preserve_original_request_identity() {
    let profile = ProfileManager::default().create("work");
    let url = Url::parse("https://example.test/data").unwrap();
    let proxy = Url::parse("http://proxy.test/forward").unwrap();
    let request_id = Uuid::new_v4();
    let mut stack = NetworkStack::default();
    stack.config.proxy = Some(proxy.clone());
    stack.route(
        "GET",
        &proxy,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 200,
            headers: BTreeMap::new(),
            body: b"via-proxy".to_vec(),
        },
    );
    let response = stack
        .fetch(NetworkRequest {
            id: request_id,
            profile,
            top_level_site: "https://example.test".into(),
            url,
            method: "GET".into(),
            headers: BTreeMap::new(),
            body: None,
            initiator: None,
        })
        .unwrap();
    assert_eq!(response.body, b"via-proxy");
    assert_eq!(response.request_id, request_id);
}

#[test]
fn configured_certificate_allowlist_rejects_unknown_https_hosts() {
    let profile = ProfileManager::default().create("work");
    let mut stack = NetworkStack::default();
    stack.config.trusted_certificates = vec!["trusted.test".into()];
    let result = stack.fetch(NetworkRequest {
        id: Uuid::new_v4(),
        profile,
        top_level_site: "https://unknown.test".into(),
        url: Url::parse("https://unknown.test/data").unwrap(),
        method: "GET".into(),
        headers: BTreeMap::new(),
        body: None,
        initiator: None,
    });
    assert_eq!(result, Err(NetworkError::UntrustedCertificate));
}

#[test]
fn worker_fetches_are_explicitly_gated_and_share_request_lifecycle() {
    let profile = ProfileManager::default().create("work");
    let url = Url::parse("https://example.test/worker-data").unwrap();
    let mut stack = NetworkStack::default();
    let request = || NetworkRequest {
        id: Uuid::new_v4(),
        profile: profile.clone(),
        top_level_site: "https://example.test".into(),
        url: url.clone(),
        method: "GET".into(),
        headers: BTreeMap::new(),
        body: None,
        initiator: None,
    };
    assert_eq!(
        stack.fetch_worker(request()),
        Err(NetworkError::WorkersDisabled)
    );
    stack.config.workers_enabled = true;
    stack.route(
        "GET",
        &url,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 200,
            headers: BTreeMap::new(),
            body: b"worker".to_vec(),
        },
    );
    assert_eq!(stack.fetch_worker(request()).unwrap().body, b"worker");
    assert!(stack.events().iter().any(|event| matches!(
        event,
        NetworkEvent::RequestInitiator { initiator, .. } if initiator == "worker"
    )));
}

#[test]
fn zero_timeout_fails_before_cache_or_route_and_records_failure() {
    let profile = ProfileManager::default().create("work");
    let url = Url::parse("https://example.test/slow").unwrap();
    let mut stack = NetworkStack::default();
    stack.limits.timeout_millis = 0;
    let result = stack.fetch(NetworkRequest {
        id: Uuid::new_v4(),
        profile,
        top_level_site: "https://example.test".into(),
        url,
        method: "GET".into(),
        headers: BTreeMap::new(),
        body: None,
        initiator: None,
    });
    assert_eq!(result, Err(NetworkError::Timeout));
    assert_eq!(stack.metrics.requests_failed, 1);
    assert!(matches!(
        stack.events().last(),
        Some(browsai_network::NetworkEvent::RequestFailed { .. })
    ));
}

#[test]
fn successful_non_get_requests_invalidate_same_partition_get_cache() {
    let profile = ProfileManager::default().create("work");
    let url = Url::parse("https://example.test/item").unwrap();
    let mut stack = NetworkStack::default();
    let request = |method: &str| NetworkRequest {
        id: Uuid::new_v4(),
        profile: profile.clone(),
        top_level_site: "https://example.test".into(),
        url: url.clone(),
        method: method.into(),
        headers: BTreeMap::new(),
        body: None,
        initiator: None,
    };
    stack.route(
        "GET",
        &url,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 200,
            headers: BTreeMap::new(),
            body: b"old".to_vec(),
        },
    );
    assert_eq!(stack.fetch(request("GET")).unwrap().body, b"old");
    stack.route(
        "POST",
        &url,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 204,
            headers: BTreeMap::new(),
            body: vec![],
        },
    );
    assert!(stack.fetch(request("POST")).is_ok());
    stack.route(
        "GET",
        &url,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 200,
            headers: BTreeMap::new(),
            body: b"new".to_vec(),
        },
    );
    assert_eq!(stack.fetch(request("GET")).unwrap().body, b"new");
}

#[test]
fn profile_identity_supplies_default_headers_and_survives_worker_path() {
    use browsai_engine_api::ProfileIdentity;
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("chrome");
    let identity = ProfileIdentity {
        user_agent: "Mozilla/5.0 Chrome/140".into(),
        locale: "en-US".into(),
        timezone: "UTC".into(),
        viewport: Default::default(),
        platform: "Linux x86_64".into(),
        brands: vec![
            browsai_engine_api::SecChUaBrand {
                brand: " Not A;Brand".into(),
                version: "99".into(),
            },
            browsai_engine_api::SecChUaBrand {
                brand: "Google Chrome".into(),
                version: "140".into(),
            },
        ],
        accept_language: "en-US,en;q=0.9".into(),
    };
    let mut stack = NetworkStack::default();
    stack.set_profile_identity(identity.clone());
    let url = Url::parse("https://example.test/").unwrap();
    stack.route(
        "GET",
        &url,
        NetworkResponse {
            request_id: Uuid::nil(),
            status: 200,
            headers: BTreeMap::new(),
            body: b"ok".to_vec(),
        },
    );
    let mut request = NetworkRequest {
        id: Uuid::new_v4(),
        profile,
        top_level_site: "example.test".into(),
        url: url.clone(),
        method: "GET".into(),
        headers: BTreeMap::new(),
        body: None,
        initiator: None,
    };
    NetworkStack::apply_profile_headers(&mut request.headers, Some(&identity));
    assert_eq!(
        request.headers.get("user-agent").map(String::as_str),
        Some("Mozilla/5.0 Chrome/140")
    );
    assert_eq!(
        request.headers.get("accept-language").map(String::as_str),
        Some("en-US,en;q=0.9")
    );
    assert!(request.headers.contains_key("sec-ch-ua"));
    assert_eq!(
        request.headers.get("sec-ch-ua-mobile").map(String::as_str),
        Some("?0")
    );
    assert_eq!(
        request.headers.get("sec-ch-ua-platform").map(String::as_str),
        Some("\"Linux x86_64\"")
    );
    request.headers.insert("user-agent".into(), "caller-supplied".into());
    NetworkStack::apply_profile_headers(&mut request.headers, Some(&identity));
    assert_eq!(
        request.headers.get("user-agent").map(String::as_str),
        Some("caller-supplied")
    );
    let _ = stack.fetch_worker(request);
}
