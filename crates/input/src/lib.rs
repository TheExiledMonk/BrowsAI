use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ActionType {
    Activate,
    Click,
    DoubleClick,
    ContextClick,
    Hover,
    Focus,
    Blur,
    Type,
    ReplaceText,
    KeyPress,
    Select,
    Toggle,
    Scroll,
    Drag,
    Drop,
    Upload,
    Download,
    Navigate,
    Back,
    Forward,
    Reload,
    Submit,
    Authenticate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentAction {
    pub id: String,
    pub target: String,
    pub action_type: ActionType,
    pub parameters: serde_json::Value,
}

/// Native input event the real runtime forwards to Servo's
/// `WebView::notify_input_event`. Tagged JSON shape over the wire so
/// HTTP / IPC clients can dispatch a single event (or a sequence) by
/// serialising only the fields each variant needs:
///
/// ```json
/// {"type": "pointer_move", "x": 100.0, "y": 200.0}
/// {"type": "scroll",       "delta_x": 0.0, "delta_y": 500.0}
/// {"type": "key_down",     "key": "Enter"}
/// ```
///
/// The `tag = "type"` / `rename_all = "snake_case"` derives keep the
/// Rust enum names stable while giving JSON callers the conventional
/// snake_case form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NativeInputEvent {
    PointerMove { x: f64, y: f64 },
    PointerDown { button: u8 },
    PointerUp { button: u8 },
    KeyDown { key: String },
    KeyUp { key: String },
    TextInput { text: String },
    Scroll { delta_x: f64, delta_y: f64 },
}

pub trait NativeInputDispatcher {
    type Error;
    fn dispatch(&mut self, event: NativeInputEvent) -> Result<(), Self::Error>;
}

/// Tunable parameters for `generate_human_trajectory`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MouseTrajectoryOptions {
    /// Approximate total duration of the move (in milliseconds).
    pub duration_millis: u64,
    /// Number of `PointerMove` events to emit before the click. Higher
    /// values produce smoother paths; lower values are cheaper.
    pub steps: usize,
    /// Maximum perpendicular offset of the two bezier control points, as a
    /// fraction of the straight-line distance. Zero collapses to a straight
    /// line.
    pub curve_amplitude: f64,
    /// Magnitude of the small overshoot past the target before settling, in
    /// pixels. Set to 0 to disable overshoot.
    pub overshoot_pixels: f64,
    /// Per-event positional jitter, in pixels, applied after the bezier
    /// sample. Set to 0 for a clean curve.
    pub jitter_pixels: f64,
    /// Pause before the click is dispatched, (ms). Useful when the page
    /// uses hover animations.
    pub pre_click_pause_millis: u64,
}

impl Default for MouseTrajectoryOptions {
    fn default() -> Self {
        Self {
            duration_millis: 600,
            steps: 32,
            curve_amplitude: 0.18,
            overshoot_pixels: 4.0,
            jitter_pixels: 0.6,
            pre_click_pause_millis: 80,
        }
    }
}

/// Humanized mouse trajectory: a sequence of `PointerMove` events along a
/// perturbed bezier curve from `start` to `end`, optionally with a small
/// overshoot near the end, followed by `PointerDown` and `PointerUp`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HumanMouseTrajectory {
    pub events: Vec<NativeInputEvent>,
}

fn bezier_point(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    t: f64,
) -> (f64, f64) {
    let one_minus_t = 1.0 - t;
    let a = one_minus_t * one_minus_t * one_minus_t;
    let b = 3.0 * one_minus_t * one_minus_t * t;
    let c = 3.0 * one_minus_t * t * t;
    let d = t * t * t;
    (
        a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
        a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
    )
}

fn ease_human(t: f64) -> f64 {
    let clamped = t.clamp(0.0, 1.0);
    let ease_out = 1.0 - (1.0 - clamped).powi(3);
    let ease_in = clamped * clamped;
    0.5 * ease_in + 0.5 * ease_out
}

/// Pseudo-random number generator with a deterministic seed. Used so the
/// trajectory is reproducible for a given `(start, end, options.seed)`.
/// Exposed so applications can derive other per-page random values
/// (e.g. an initial cursor position) from the same seed stream.
#[derive(Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    pub fn next_unit(&mut self) -> f64 {
        let v = self.next_u64() >> 11;
        (v as f64) / ((1u64 << 53) as f64)
    }
    pub fn next_symmetric(&mut self) -> f64 {
        self.next_unit() * 2.0 - 1.0
    }
}

fn perpendicular_offset(start: (f64, f64), end: (f64, f64), amount: f64) -> (f64, f64) {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    (-dy / len * amount, dx / len * amount)
}

/// Generate a humanized mouse trajectory from `start` to `end`. The path
/// is a cubic bezier with two control points offset perpendicular to the
/// start→end line by `options.curve_amplitude * distance`. Per-step
/// timing follows an acceleration/deceleration curve. A small overshoot
/// is added near the end when `options.overshoot_pixels > 0`. The cursor
/// jitter by `options.jitter_pixels` per step. The sequence ends with a
/// `PointerDown` then `PointerUp`.
pub fn generate_human_trajectory(
    start: (f64, f64),
    end: (f64, f64),
    options: MouseTrajectoryOptions,
) -> HumanMouseTrajectory {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let distance = (dx * dx + dy * dy).sqrt();
    let seed = (start.0.to_bits())
        .wrapping_mul(0x9E3779B97F4A7C15)
        .wrapping_add(start.1.to_bits())
        .wrapping_add(end.0.to_bits())
        .wrapping_mul(0xBF58476D1CE4E5B9)
        .wrapping_add(end.1.to_bits());
    let mut rng = SplitMix64::new(seed);
    let amp = options.curve_amplitude.clamp(0.0, 1.0);
    let offset1 = perpendicular_offset(start, end, distance * amp * (0.6 + rng.next_unit() * 0.4));
    let offset2 = perpendicular_offset(
        start,
        end,
        -(distance * amp * (0.6 + rng.next_unit() * 0.4)),
    );
    let p0 = start;
    let p3 = end;
    let p1 = (
        start.0 + dx * 0.25 + offset1.0,
        start.1 + dy * 0.25 + offset1.1,
    );
    let p2 = (
        start.0 + dx * 0.75 + offset2.0,
        start.1 + dy * 0.75 + offset2.1,
    );
    let steps = options.steps.max(4);
    let mut events: Vec<NativeInputEvent> = Vec::with_capacity(steps + 4);
    let overshoot = options.overshoot_pixels.max(0.0);
    let jitter = options.jitter_pixels.max(0.0);
    let overshoot_target = if overshoot > 0.0 {
        let len = distance.max(1.0);
        let dir = (dx / len, dy / len);
        let perp = (-dir.1, dir.0);
        let side = if rng.next_symmetric() >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let overshoot_amount = overshoot * (0.6 + rng.next_unit() * 0.8);
        (
            end.0 + dir.0 * overshoot_amount * 0.4 + perp.0 * overshoot_amount * 0.6 * side,
            end.1 + dir.1 * overshoot_amount * 0.4 + perp.1 * overshoot_amount * 0.6 * side,
        )
    } else {
        end
    };
    for i in 0..steps {
        let linear = i as f64 / (steps as f64 - 1.0);
        let eased = ease_human(linear);
        let mut point = bezier_point(p0, p1, p2, p3, eased);
        if overshoot > 0.0 && linear > 0.78 && linear < 0.95 {
            let blend = ((linear - 0.78) / 0.17).clamp(0.0, 1.0);
            point = (
                point.0 + (overshoot_target.0 - point.0) * blend,
                point.1 + (overshoot_target.1 - point.1) * blend,
            );
        }
        if overshoot > 0.0 && linear >= 0.95 {
            let settle = ((linear - 0.95) / 0.05).clamp(0.0, 1.0);
            point = (
                overshoot_target.0 + (end.0 - overshoot_target.0) * settle,
                overshoot_target.1 + (end.1 - overshoot_target.1) * settle,
            );
        }
        if jitter > 0.0 {
            point = (
                point.0 + rng.next_symmetric() * jitter,
                point.1 + rng.next_symmetric() * jitter,
            );
        }
        events.push(NativeInputEvent::PointerMove {
            x: point.0,
            y: point.1,
        });
    }
    let _ = options.duration_millis;
    let _ = options.pre_click_pause_millis;
    events.push(NativeInputEvent::PointerDown { button: 0 });
    events.push(NativeInputEvent::PointerUp { button: 0 });
    HumanMouseTrajectory { events }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trajectory_starts_at_start_and_ends_near_end() {
        let options = MouseTrajectoryOptions {
            jitter_pixels: 0.0,
            overshoot_pixels: 0.0,
            ..MouseTrajectoryOptions::default()
        };
        let start = (10.0, 20.0);
        let end = (300.0, 220.0);
        let trajectory = generate_human_trajectory(start, end, options);
        let first = trajectory.events.first().expect("at least one event");
        match first {
            NativeInputEvent::PointerMove { x, y } => {
                assert!((x - start.0).abs() < 0.01);
                assert!((y - start.1).abs() < 0.01);
            }
            _ => panic!("first event must be PointerMove"),
        }
        let move_events: Vec<_> = trajectory
            .events
            .iter()
            .filter_map(|e| match e {
                NativeInputEvent::PointerMove { x, y } => Some((*x, *y)),
                _ => None,
            })
            .collect();
        let last_move = move_events.last().expect("at least one move");
        assert!((last_move.0 - end.0).abs() < 0.5);
        assert!((last_move.1 - end.1).abs() < 0.5);
        assert!(matches!(
            trajectory.events[trajectory.events.len() - 2],
            NativeInputEvent::PointerDown { .. }
        ));
        assert!(matches!(
            trajectory.events[trajectory.events.len() - 1],
            NativeInputEvent::PointerUp { .. }
        ));
    }

    #[test]
    fn trajectory_is_not_a_straight_line_when_curve_amplitude_is_nonzero() {
        let options = MouseTrajectoryOptions {
            curve_amplitude: 0.25,
            jitter_pixels: 0.0,
            overshoot_pixels: 0.0,
            steps: 16,
            ..MouseTrajectoryOptions::default()
        };
        let trajectory = generate_human_trajectory((0.0, 0.0), (400.0, 0.0), options);
        let moves: Vec<(f64, f64)> = trajectory
            .events
            .iter()
            .filter_map(|e| match e {
                NativeInputEvent::PointerMove { x, y } => Some((*x, *y)),
                _ => None,
            })
            .collect();
        assert!(moves.len() >= 4);
        let midpoint = moves[moves.len() / 2];
        let straight_y = 0.0;
        assert!(
            midpoint.1.abs() > 5.0,
            "expected a curved path, midpoint y was {}",
            midpoint.1
        );
        assert!((midpoint.1).is_finite());
        assert_ne!(midpoint.1, straight_y);
    }

    #[test]
    fn trajectory_with_zero_curve_is_a_straight_line() {
        let options = MouseTrajectoryOptions {
            curve_amplitude: 0.0,
            jitter_pixels: 0.0,
            overshoot_pixels: 0.0,
            steps: 8,
            ..MouseTrajectoryOptions::default()
        };
        let trajectory = generate_human_trajectory((0.0, 0.0), (400.0, 200.0), options);
        for event in &trajectory.events {
            if let NativeInputEvent::PointerMove { x, y } = event {
                let expected_y = (200.0 / 400.0) * x;
                assert!((y - expected_y).abs() < 0.001);
            }
        }
    }

    #[test]
    fn trajectory_is_deterministic_for_same_start_end() {
        let options = MouseTrajectoryOptions::default();
        let a = generate_human_trajectory((10.0, 20.0), (300.0, 220.0), options);
        let b = generate_human_trajectory((10.0, 20.0), (300.0, 220.0), options);
        assert_eq!(a, b);
    }

    #[test]
    fn trajectory_settles_back_to_end_after_overshoot() {
        let options = MouseTrajectoryOptions {
            curve_amplitude: 0.0,
            jitter_pixels: 0.0,
            steps: 64,
            ..MouseTrajectoryOptions::default()
        };
        let end = (400.0, 0.0);
        let trajectory = generate_human_trajectory((0.0, 0.0), end, options);
        let moves: Vec<(f64, f64)> = trajectory
            .events
            .iter()
            .filter_map(|e| match e {
                NativeInputEvent::PointerMove { x, y } => Some((*x, *y)),
                _ => None,
            })
            .collect();
        let settle_radius = options.overshoot_pixels + 1.0;
        for (i, (x, y)) in moves.iter().enumerate() {
            let linear = i as f64 / (moves.len() as f64 - 1.0);
            if linear < 0.94 {
                continue;
            }
            let dx = x - end.0;
            let dy = y - end.1;
            let d = (dx * dx + dy * dy).sqrt();
            assert!(
                d <= settle_radius,
                "trajectory did not settle at the end (linear={linear}, distance={d}, bound={settle_radius})"
            );
        }
    }

    #[test]
    fn native_input_event_serialises_with_snake_case_type_tag() {
        // The HTTP /native-input endpoint relies on the tagged
        // internally-tagged representation: each variant emits a
        // "type" field with snake_case names, and the variant fields
        // are flattened into the same object. Round-trip both
        // directions so a future change to the derive can't quietly
        // break the wire format.
        let cases = [
            (
                NativeInputEvent::PointerMove { x: 12.5, y: -3.0 },
                r#"{"type":"pointer_move","x":12.5,"y":-3.0}"#,
            ),
            (
                NativeInputEvent::PointerDown { button: 0 },
                r#"{"type":"pointer_down","button":0}"#,
            ),
            (
                NativeInputEvent::PointerUp { button: 2 },
                r#"{"type":"pointer_up","button":2}"#,
            ),
            (
                NativeInputEvent::KeyDown {
                    key: "Enter".into(),
                },
                r#"{"type":"key_down","key":"Enter"}"#,
            ),
            (
                NativeInputEvent::KeyUp {
                    key: "Escape".into(),
                },
                r#"{"type":"key_up","key":"Escape"}"#,
            ),
            (
                NativeInputEvent::TextInput { text: "hi".into() },
                r#"{"type":"text_input","text":"hi"}"#,
            ),
            (
                NativeInputEvent::Scroll {
                    delta_x: 1.0,
                    delta_y: -2.5,
                },
                r#"{"type":"scroll","delta_x":1.0,"delta_y":-2.5}"#,
            ),
        ];
        for (event, expected_json) in cases {
            let actual = serde_json::to_string(&event).expect("serialise");
            assert_eq!(actual, expected_json, "serialised shape for {event:?}");
            let round_tripped: NativeInputEvent =
                serde_json::from_str(expected_json).expect("deserialise");
            assert_eq!(round_tripped, event, "round-trip for {event:?}");
        }
    }

    #[test]
    fn native_input_event_unknown_variant_returns_error() {
        // The /native-input endpoint surfaces serde_json's parse
        // error verbatim so callers can spot a typo'd `type` value
        // without silently dropping the request. Lock that contract
        // in.
        let bad = serde_json::from_str::<NativeInputEvent>(r#"{"type":"wiggle"}"#);
        let error = bad.expect_err("unknown variant must fail");
        let message = error.to_string();
        assert!(
            message.contains("wiggle") || message.contains("unknown variant"),
            "unexpected error message: {message}"
        );
    }
}
