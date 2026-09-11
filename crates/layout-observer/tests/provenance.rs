use browsai_layout_observer::{LayoutBox, LayoutSnapshot, Rect};
use browsai_provenance::{ProvenanceSource, SourceKind};

#[test]
fn layout_boxes_preserve_geometry_source_provenance() {
    let mut snapshot = LayoutSnapshot::default();
    snapshot.insert(LayoutBox {
        node_id: 7,
        rect: Rect {
            x: 1.0,
            y: 2.0,
            width: 30.0,
            height: 10.0,
        },
        visible: true,
        clipped: false,
        z_index: 0,
        pointer_events: true,
        disabled: false,
        provenance: vec![ProvenanceSource {
            kind: SourceKind::Layout,
            reference: "engine:layout-generation-4/node-7".into(),
            detail: None,
        }],
    });
    let encoded = serde_json::to_string(&snapshot).unwrap();
    assert!(encoded.contains("engine:layout-generation-4/node-7"));
    assert_eq!(
        snapshot.get(7).unwrap().provenance[0].kind,
        SourceKind::Layout
    );
}
