use browsai_agent_tree::{AgentNode, AgentRenderTree, ApplicationType, NodeState, StructuralRole};
use browsai_api_discovery::{CausalEvidence, NetworkTiming};
use browsai_application_ir::{
    ApplicationCollection, ApplicationDialog, ApplicationEntity, ApplicationForm, ApplicationIr,
    ApplicationNotification, ApplicationQuery, ApplicationRelation, ApplicationTable, SearchResult,
    SemanticCache, SiteLearningCache, Workflow, WorkflowStep,
};
use browsai_provenance::Confidence;
use browsai_provenance::{ProvenanceSource, SourceKind};
use uuid::Uuid;

#[test]
fn application_ir_preserves_source_nodes_and_confidence() {
    let mut tree = AgentRenderTree::new_page("Example");
    tree.nodes.push(AgentNode {
        id: "dom:9".into(),
        origin: None,
        identity_key: Some("invoice:1".into()),
        structural_role: StructuralRole::Region,
        semantic_role: None,
        application_type: Some(ApplicationType::Invoice),
        name: Some("INV-1".into()),
        value: None,
        description: None,
        state: NodeState::default(),
        geometry: None,
        relationships: vec![],
        actions: vec![],
        children: vec![],
        provenance: vec![],
        confidence: Confidence::PROBABLE,
        generation: 0,
    });
    let app = ApplicationIr::from_semantic(tree);
    assert_eq!(app.graph.entities[0].id, "invoice:1");
    assert_eq!(app.graph.entities[0].source_nodes, vec!["dom:9"]);
    assert_eq!(app.graph.entities[0].confidence, Confidence::PROBABLE);
}

#[test]
fn application_ir_carries_causal_network_evidence() {
    let tree = AgentRenderTree::new_page("Example");
    let request_id = Uuid::new_v4();
    let evidence = CausalEvidence {
        request_id,
        entity_id: Some("invoice:1".into()),
        action_id: Some("action:save".into()),
        state_change: Some("invoice-updated".into()),
        form_id: Some("invoice-form".into()),
        script_id: Some("save-handler".into()),
        timing: NetworkTiming {
            started_at_tick: 4,
            completed_at_tick: 9,
        },
        provenance: vec![ProvenanceSource {
            kind: SourceKind::NetworkRequest,
            reference: request_id.to_string(),
            detail: None,
        }],
    };
    let app = ApplicationIr::from_semantic_with_evidence(tree, vec![evidence.clone()]);
    assert_eq!(app.causal_evidence, vec![evidence]);
    assert_eq!(app.causal_evidence[0].timing.completed_at_tick, 9);
    let report = app.explainability_report("invoice:1");
    assert_eq!(report.evidence.len(), 1);
    assert_eq!(report.evidence[0].observed_at_millis, 9);
    assert!(report.evidence[0]
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains("elapsed_ticks=5")));
}

#[test]
fn application_graph_models_relations_and_virtualized_collections() {
    let tree = AgentRenderTree::new_page("Example");
    let mut ir = ApplicationIr::from_semantic(tree);
    ir.add_relation(ApplicationRelation {
        from: "order:1".into(),
        relation: "contains".into(),
        to: "line:1".into(),
    });
    ir.add_relation(ApplicationRelation {
        from: "order:1".into(),
        relation: "contains".into(),
        to: "line:1".into(),
    });
    ir.add_collection(ApplicationCollection {
        id: "orders".into(),
        kind: Some(ApplicationType::Order),
        members: vec!["order:1".into()],
        total_known: Some(100),
        virtualized: true,
    });
    ir.graph.entities.push(ApplicationEntity {
        id: "order:1".into(),
        kind: ApplicationType::Order,
        fields: Default::default(),
        source_nodes: vec![],
        confidence: Confidence::DIRECT,
        provenance: vec![],
    });
    ir.graph.collections[0].members.push("order:1".into());
    assert_eq!(ir.graph.relations.len(), 1);
    assert!(ir.collection("orders").unwrap().virtualized);
    assert_eq!(
        ir.query_entities(&ApplicationQuery {
            kind: Some(ApplicationType::Order),
            collection_id: Some("orders".into()),
            ..Default::default()
        })
        .len(),
        1
    );
    let page = ir.query_entities_page(
        &ApplicationQuery {
            kind: Some(ApplicationType::Order),
            collection_id: Some("orders".into()),
            ..Default::default()
        },
        0,
        1,
    );
    assert_eq!(page.results.len(), 1);
    assert_eq!(page.total, 1);
    assert!(!page.truncated);
    assert_eq!(page.next_cursor, None);
    assert_eq!(
        serde_json::to_value(&ir).unwrap()["graph"]["collections"][0]["total_known"],
        100
    );
}

#[test]
fn application_entity_pages_bound_large_collection_results() {
    let mut ir = ApplicationIr::from_semantic(AgentRenderTree::new_page("Example"));
    ir.add_collection(ApplicationCollection {
        id: "orders".into(),
        kind: Some(ApplicationType::Order),
        members: vec![],
        total_known: Some(7),
        virtualized: true,
    });
    for index in 0..7 {
        let id = format!("row:{index}");
        ir.graph.entities.push(ApplicationEntity {
            id: id.clone(),
            kind: ApplicationType::Order,
            fields: Default::default(),
            source_nodes: vec![],
            confidence: Confidence::DIRECT,
            provenance: vec![],
        });
        ir.graph.collections[0].members.push(id);
    }
    let query = ApplicationQuery {
        collection_id: Some(ir.graph.collections[0].id.clone()),
        ..Default::default()
    };
    let first = ir.query_entities_page(&query, 0, 3);
    assert_eq!(first.results.len(), 3);
    assert_eq!(first.total, 7);
    assert!(first.truncated);
    assert_eq!(first.next_offset, Some(3));
    assert_eq!(first.next_cursor.as_deref(), Some("3"));

    let second = ir.query_entities_page(&query, 3, 3);
    assert_eq!(second.results.len(), 3);
    assert_eq!(second.next_cursor.as_deref(), Some("6"));

    let final_page = ir.query_entities_page(&query, 6, 3);
    assert_eq!(final_page.results.len(), 1);
    assert!(!final_page.truncated);
    assert_eq!(final_page.next_offset, None);
    assert_eq!(final_page.next_cursor, None);
}

#[test]
fn site_learning_and_semantic_cache_are_bounded_and_deterministic() {
    let tree = AgentRenderTree::new_page("Example");
    let mut learning = SiteLearningCache::with_capacity(1);
    learning.observe("https://one.test", &tree);
    assert_eq!(learning.get("https://one.test").unwrap().observations, 1);
    learning.observe("https://two.test", &tree);
    assert_eq!(learning.len(), 1);

    let mut cache = SemanticCache::with_capacity(2);
    cache.insert("first", tree.clone());
    cache.insert("second", tree.clone());
    assert!(cache.get("first").is_some());
    cache.insert("third", tree);
    assert_eq!(cache.len(), 2);
    assert!(cache.get("second").is_none());
}

#[test]
fn application_surface_models_round_trip() {
    let value = serde_json::json!({
        "workflow": Workflow {
            id: "checkout".into(),
            steps: vec![WorkflowStep { action: "submit".into(), target: Some("form:checkout".into()) }]
        },
        "search": SearchResult { id: "order:1".into(), title: "Order 1".into(), score: Some(0.9) },
        "table": ApplicationTable { id: "orders".into(), columns: vec!["id".into()], rows: vec![vec!["order:1".into()]] },
        "form": ApplicationForm { id: "checkout".into(), fields: [("email".into(), "text".into())].into_iter().collect() },
        "dialog": ApplicationDialog { id: "confirm".into(), title: "Confirm".into(), modal: true },
        "notification": ApplicationNotification { id: "saved".into(), message: "Saved".into(), level: "success".into() }
    });
    assert_eq!(value["dialog"]["modal"], true);
    assert_eq!(value["table"]["rows"][0][0], "order:1");
}
