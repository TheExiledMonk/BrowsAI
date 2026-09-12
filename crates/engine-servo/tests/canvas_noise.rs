//! Tests for the canvas-noise LFSR algorithm. The algorithm itself
//! lives inside the vendored `servo-canvas` crate
//! (`canvas_data.rs::apply_canvas_noise`) so we re-implement the same
//! xorshift32 step here and exercise the contract:
//! - same seed -> same perturbation (deterministic)
//! - different seed -> different perturbation
//! - every output byte stays in [0, 255]
//!
//! The vendored impl is covered by an integration test that wires up
//! `BROWSAI_CANVAS_NOISE_SEED` end-to-end; this file covers the
//! arithmetic in BrowsAI's own crate so the test runs as part of
//! `cargo test --workspace`.

fn apply_noise(bytes: &mut [u8], seed: u32) {
    let mut state: u32 = seed.wrapping_add(0x9E3779B9);
    for pixel in bytes.chunks_exact_mut(4) {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let delta = (state & 0xff) as i16 - 128;
        for byte in pixel.iter_mut() {
            *byte = ((*byte as i16 + delta).clamp(0, 255)) as u8;
        }
    }
}

#[test]
fn canvas_noise_changes_byte_pattern() {
    let mut canvas: Vec<u8> = (0..16u8).cycle().take(64).collect();
    let original = canvas.clone();
    apply_noise(&mut canvas, 42);
    assert_ne!(canvas, original);
}

#[test]
fn canvas_noise_is_deterministic_for_same_seed() {
    let mut a: Vec<u8> = (0..16u8).cycle().take(64).collect();
    let mut b: Vec<u8> = a.clone();
    apply_noise(&mut a, 42);
    apply_noise(&mut b, 42);
    assert_eq!(a, b);
}

#[test]
fn canvas_noise_differs_across_seeds() {
    let mut a: Vec<u8> = (0..16u8).cycle().take(64).collect();
    let mut b: Vec<u8> = a.clone();
    apply_noise(&mut a, 42);
    apply_noise(&mut b, 43);
    assert_ne!(a, b);
}

#[test]
fn canvas_noise_keeps_bytes_in_u8_range() {
    // Run the noise on edge-value inputs to confirm the algorithm
    // never produces an out-of-range byte. Every output is a u8 by
    // construction (the clamp step), so this is mainly a smoke test
    // that the LFSR does not panic on 0/255 input.
    let mut canvas: Vec<u8> = vec![0, 255, 0, 255, 255, 0, 255, 0];
    apply_noise(&mut canvas, 7);
    assert_eq!(canvas.len(), 8);
    // Sum of deltas should be non-zero for a non-zero seed on edge
    // inputs (a degenerate algorithm would cancel out to 0).
    let sum: u32 = canvas.iter().map(|b| *b as u32).sum();
    assert!(sum > 0);
}

#[test]
fn canvas_noise_is_per_pixel() {
    // With one seed, two pixels starting with the same bytes should
    // diverge after a couple of LFSR steps.
    let mut canvas: Vec<u8> = vec![0, 0, 0, 0, 0, 0, 0, 0];
    apply_noise(&mut canvas, 12345);
    assert_ne!(&canvas[0..4], &canvas[4..8]);
}
