use browsai_agent_protocol::{AgentOperation, GeometryConstraint, PageQuery, Query, QueryShape};
use browsai_agent_tree::{AgentNode, AgentRenderTree, NodeState, SemanticRole, StructuralRole};
use browsai_provenance::Confidence;

#[test]
fn query_and_explain_stay_structured() {
    let mut tree = AgentRenderTree::new_page("Example");
    tree.nodes.push(AgentNode {
        id: "dom:2".into(),
        origin: Some("https://example.test".into()),
        identity_key: Some("button:save".into()),
        structural_role: StructuralRole::Button,
        semantic_role: Some(SemanticRole::SubmitAction),
        application_type: None,
        name: Some("Save".into()),
        value: None,
        description: None,
        state: NodeState {
            visible: true,
            enabled: true,
            ..Default::default()
        },
        geometry: None,
        relationships: vec![],
        actions: vec![],
        children: vec![],
        provenance: vec![],
        confidence: Confidence::DIRECT,
        generation: 0,
    });
    let page = PageQuery::new(&tree);
    let result = page.query(&Query {
        semantic_role: Some(SemanticRole::SubmitAction),
        ..Default::default()
    });
    assert_eq!(result.len(), 1);
    assert_eq!(
        page.explain("dom:2").unwrap().confidence,
        Confidence::DIRECT
    );
}

#[test]
fn leases_are_exclusive_expiring_and_interruptible() {
    use browsai_agent_protocol::{LeaseManager, LeaseScope, ProtocolError};
    let mut leases = LeaseManager::default();
    let first = leases
        .acquire("agent-a", LeaseScope::Page("tab-1".into()), 5, 10)
        .unwrap();
    assert_eq!(
        leases.acquire("agent-b", LeaseScope::Page("tab-1".into()), 5, 10),
        Err(ProtocolError::LeaseHeld)
    );
    assert_eq!(
        leases.release(first.id, "agent-b"),
        Err(ProtocolError::LeaseOwnerMismatch)
    );
    leases.interrupt("agent-a", 11);
    assert!(leases.is_interrupted("agent-a"));
    assert_eq!(leases.active(15).count(), 0);
}

#[test]
fn agent_requests_validate_protocol_identity_and_operation_fields() {
    use browsai_agent_protocol::{AgentOperation, AgentRequest, ProtocolError, PROTOCOL_VERSION};
    let mut request = AgentRequest {
        protocol_version: PROTOCOL_VERSION,
        request_id: uuid::Uuid::new_v4(),
        agent_id: "agent".into(),
        session_id: "session".into(),
        operation: AgentOperation::Navigate { url: "".into() },
    };
    assert_eq!(request.validate(), Err(ProtocolError::InvalidRequest));
    request.operation = AgentOperation::Navigate {
        url: "https://example.test".into(),
    };
    assert_eq!(request.validate(), Ok(()));
    request.operation = AgentOperation::Navigate {
        url: "not a url".into(),
    };
    assert_eq!(request.validate(), Err(ProtocolError::InvalidUrl));
    request.protocol_version += 1;
    assert_eq!(
        request.validate(),
        Err(ProtocolError::VersionMismatch {
            expected: PROTOCOL_VERSION,
            actual: PROTOCOL_VERSION + 1,
        })
    );
}

#[test]
fn nested_agent_operations_use_camel_case_wire_names() {
    let operation = AgentOperation::AcquireLease {
        scope: browsai_agent_protocol::LeaseScope::Page("page-1".into()),
        ttl_ticks: 5,
    };
    let encoded = serde_json::to_value(operation).unwrap();
    assert_eq!(encoded["acquireLease"]["ttlTicks"], 5);
}

#[test]
fn protocol_query_wire_format_matches_sdk_camel_case() {
    let query = Query {
        semantic_role: Some(SemanticRole::SubmitAction),
        identity_key: Some("button:save".into()),
        ..Default::default()
    };
    let value = serde_json::to_value(query).unwrap();
    assert_eq!(value["semanticRole"], "SubmitAction");
    assert_eq!(value["identityKey"], "button:save");
    assert!(value.get("semantic_role").is_none());
}

#[test]
fn query_matches_state_geometry_and_relationship_constraints() {
    let mut tree = AgentRenderTree::new_page("Example");
    tree.nodes.push(AgentNode {
        id: "dom:2".into(),
        origin: Some("https://example.test".into()),
        identity_key: None,
        structural_role: StructuralRole::Button,
        semantic_role: None,
        application_type: None,
        name: Some("Save".into()),
        value: None,
        description: None,
        state: NodeState {
            visible: true,
            enabled: true,
            focused: true,
            selected: true,
            ..Default::default()
        },
        geometry: Some(browsai_agent_tree::Geometry {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 40.0,
        }),
        relationships: vec![browsai_agent_tree::Relationship {
            kind: "labels".into(),
            target: "dom:1".into(),
        }],
        actions: vec![],
        children: vec![],
        provenance: vec![],
        confidence: Confidence::DIRECT,
        generation: 0,
    });
    let query = Query {
        name_contains: Some("av".into()),
        focused: Some(true),
        selected: Some(true),
        geometry: Some(GeometryConstraint {
            min_x: Some(0.0),
            max_x: Some(20.0),
            min_y: Some(10.0),
            max_y: Some(30.0),
            min_width: Some(80.0),
            min_height: Some(30.0),
        }),
        relationship_kind: Some("labels".into()),
        relationship_target: Some("dom:1".into()),
        origin: Some("https://example.test".into()),
        ..Default::default()
    };
    assert_eq!(PageQuery::new(&tree).query(&query).len(), 1);
    assert_eq!(PageQuery::new(&tree).search("av").len(), 1);
    let encoded = serde_json::to_value(query).unwrap();
    assert_eq!(encoded["relationshipKind"], "labels");
    assert_eq!(encoded["geometry"]["minWidth"], 80.0);
    assert_eq!(encoded["origin"], "https://example.test");
    assert!(encoded.get("relationship_kind").is_none());
    assert_eq!(encoded["nameContains"], "av");
}

#[test]
fn query_resolution_reports_ambiguity_and_stale_generation() {
    use browsai_agent_protocol::{QueryResolutionError, QueryResult};
    let mut tree = AgentRenderTree::new_page("Example");
    tree.generation = 7;
    for id in ["one", "two"] {
        tree.nodes.push(AgentNode {
            id: id.into(),
            origin: None,
            identity_key: Some("duplicate".into()),
            structural_role: StructuralRole::Button,
            semantic_role: None,
            application_type: None,
            name: Some("Save".into()),
            value: None,
            description: None,
            state: NodeState {
                visible: true,
                enabled: true,
                ..Default::default()
            },
            geometry: None,
            relationships: vec![],
            actions: vec![],
            children: vec![],
            provenance: vec![],
            confidence: Confidence::DIRECT,
            generation: 7,
        });
    }
    let page = PageQuery::new(&tree);
    let query = Query {
        identity_key: Some("duplicate".into()),
        ..Default::default()
    };
    assert_eq!(
        page.resolve(&query),
        Err(QueryResolutionError::Ambiguous { matches: 2 })
    );
    assert_eq!(
        page.resolve_at_generation(&Query::default(), 6),
        Err(QueryResolutionError::StaleReference {
            expected: 6,
            actual: 7,
        })
    );
    let _typed: Result<QueryResult, QueryResolutionError> = page.resolve_at_generation(
        &Query {
            role: Some(StructuralRole::Button),
            identity_key: Some("missing".into()),
            ..Default::default()
        },
        7,
    );
}

#[test]
fn page_query_exposes_bounded_pages_and_cursors() {
    let mut tree = AgentRenderTree::new_page("Example");
    for index in 0..3 {
        tree.nodes.push(AgentNode {
            id: format!("dom:{index}"),
            origin: None,
            identity_key: None,
            structural_role: StructuralRole::Text,
            semantic_role: None,
            application_type: None,
            name: Some(format!("item-{index}")),
            value: None,
            description: None,
            state: NodeState::default(),
            geometry: None,
            relationships: vec![],
            actions: vec![],
            children: vec![],
            provenance: vec![],
            confidence: Confidence::DIRECT,
            generation: 0,
        });
    }
    let page = PageQuery::new(&tree).render_page(1, 1);
    assert_eq!(page.total, 4);
    assert_eq!(page.cursor, 1);
    assert_eq!(page.limit, 1);
    assert_eq!(page.offset, 1);
    assert_eq!(page.next_offset, Some(2));
    assert!(page.truncated);
    assert_eq!(page.next_cursor.as_deref(), Some("2"));
    assert_eq!(page.results[0].name.as_deref(), Some("item-0"));
    let zero_limit = PageQuery::new(&tree).render_page(0, 0);
    assert_eq!(zero_limit.results.len(), 1);
    assert_eq!(zero_limit.limit, 1);
    assert_eq!(zero_limit.next_offset, Some(1));
    assert!(zero_limit.truncated);
    assert_eq!(zero_limit.next_cursor.as_deref(), Some("1"));
}

#[test]
fn shaped_queries_bound_tokens_and_optional_context() {
    let mut tree = AgentRenderTree::new_page("Example");
    tree.nodes[0]
        .provenance
        .push(browsai_provenance::ProvenanceSource {
            kind: browsai_provenance::SourceKind::Dom,
            reference: "page".into(),
            detail: None,
        });
    let results = PageQuery::new(&tree).query_shaped(
        &Query::default(),
        QueryShape {
            max_results: Some(1),
            max_tokens: Some(1_000),
            include_value: false,
            include_provenance: false,
            include_geometry: false,
        },
    );
    assert_eq!(results.len(), 1);
    assert!(results[0].value.is_none());
    assert!(results[0].provenance.is_empty());
    assert!(results[0].geometry.is_none());
}
