use browsai_layout_observer::{LayoutBox, LayoutSnapshot, Rect};
use serde::Deserialize;

#[derive(Deserialize)]
struct Golden {
    viewport: Size,
    point: Point,
    expected_hit: u64,
    boxes: Vec<GoldenBox>,
}

#[derive(Deserialize)]
struct Size {
    width: f64,
    height: f64,
}

#[derive(Deserialize)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
struct GoldenBox {
    node_id: u64,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    z_index: i32,
}

#[test]
fn layout_hit_testing_matches_repository_golden_fixture() {
    let golden: Golden = serde_json::from_str(include_str!(
        "../../../test-sites/semantic-golden/layout-hit.json"
    ))
    .unwrap();
    let mut snapshot = LayoutSnapshot {
        viewport: Rect {
            x: 0.0,
            y: 0.0,
            width: golden.viewport.width,
            height: golden.viewport.height,
        },
        ..Default::default()
    };
    for item in golden.boxes {
        snapshot.insert(LayoutBox {
            node_id: item.node_id,
            rect: Rect {
                x: item.x,
                y: item.y,
                width: item.width,
                height: item.height,
            },
            visible: true,
            clipped: false,
            z_index: item.z_index,
            pointer_events: true,
            disabled: false,
            provenance: vec![],
        });
    }
    assert_eq!(
        snapshot.hit_test(golden.point.x, golden.point.y),
        Some(golden.expected_hit)
    );
}
