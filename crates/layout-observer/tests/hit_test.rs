use browsai_layout_observer::{LayoutBox, LayoutSnapshot, Rect};

#[test]
fn hit_testing_uses_visibility_and_z_order() {
    let mut snapshot = LayoutSnapshot {
        viewport: Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 300.0,
        },
        ..Default::default()
    };
    snapshot.insert(LayoutBox {
        node_id: 1,
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 100.0,
        },
        visible: true,
        clipped: false,
        z_index: 1,
        pointer_events: true,
        disabled: false,
        provenance: vec![],
    });
    snapshot.insert(LayoutBox {
        node_id: 2,
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 100.0,
        },
        visible: true,
        clipped: false,
        z_index: 2,
        pointer_events: true,
        disabled: false,
        provenance: vec![],
    });
    assert_eq!(snapshot.hit_test(20.0, 20.0), Some(2));
    assert_eq!(snapshot.hit_test(401.0, 20.0), None);
}

#[test]
fn equal_z_index_hit_testing_is_deterministic() {
    let mut snapshot = LayoutSnapshot {
        viewport: Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        ..Default::default()
    };
    for node_id in [2, 1] {
        snapshot.insert(LayoutBox {
            node_id,
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 50.0,
            },
            visible: true,
            clipped: false,
            z_index: 3,
            pointer_events: true,
            disabled: false,
            provenance: vec![],
        });
    }
    assert_eq!(snapshot.hit_test(10.0, 10.0), Some(2));
}

#[test]
fn layout_updates_report_old_and_new_dirty_regions() {
    let mut snapshot = LayoutSnapshot::default();
    let layout = LayoutBox {
        node_id: 1,
        rect: Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        visible: true,
        clipped: false,
        z_index: 0,
        pointer_events: true,
        disabled: false,
        provenance: vec![],
    };
    snapshot.insert(layout.clone());
    assert_eq!(snapshot.drain_dirty_regions(), vec![layout.rect]);
    let moved = LayoutBox {
        rect: Rect {
            x: 20.0,
            ..layout.rect
        },
        ..layout
    };
    snapshot.insert(moved.clone());
    assert_eq!(
        snapshot.drain_dirty_regions(),
        vec![
            Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            moved.rect
        ]
    );
    assert!(snapshot.invalidate(1));
    assert_eq!(snapshot.drain_dirty_regions(), vec![moved.rect]);
    assert!(snapshot.remove(1).is_some());
    assert_eq!(snapshot.drain_dirty_regions(), vec![moved.rect]);
}
