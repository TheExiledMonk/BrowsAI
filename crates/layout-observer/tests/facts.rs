use browsai_layout_observer::{LayoutBox, LayoutFacts, LayoutSnapshot, Rect};

#[test]
fn layout_snapshot_preserves_scroll_transform_overlap_and_hit_regions() {
    let mut snapshot = LayoutSnapshot::default();
    snapshot.insert(LayoutBox {
        node_id: 4,
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        visible: true,
        clipped: false,
        z_index: 1,
        pointer_events: true,
        disabled: false,
        provenance: vec![],
    });
    snapshot.set_facts(
        4,
        LayoutFacts {
            scroll_offset_x: 3.0,
            scroll_offset_y: 8.0,
            transform: [1.0, 0.0, 0.0, 1.0, 10.0, 20.0],
            overlap: vec![5],
            hit_regions: vec![Rect {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 20.0,
            }],
        },
    );
    assert_eq!(snapshot.facts(4).unwrap().scroll_offset_y, 8.0);
    assert_eq!(snapshot.facts(4).unwrap().overlap, vec![5]);
    assert_eq!(snapshot.hit_test(15.0, 15.0), Some(4));
    assert_eq!(snapshot.hit_test(80.0, 80.0), None);
}
