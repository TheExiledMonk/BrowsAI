/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#![deny(unsafe_code)]
#![allow(clippy::too_many_arguments)]

mod backend;
pub mod canvas_data;
pub mod canvas_paint_thread;

/// Returns the canvas / 2D-rendering noise seed selected by the
/// BrowsAI CLI. Reads `BROWSAI_CANVAS_NOISE_SEED` from the process
/// environment and parses it as a `u32`. Anything missing or
/// unparseable yields `None`, which signals the canvas impl to use
/// stock rendering.
///
/// The actual noise injection lives in Servo's CSS / render
/// pipeline (the vendored `stylo` crate, which BrowsAI does not
/// vendor). When the noise pipeline is wired up, this helper is the
/// single seam BrowsAI exposes.
pub fn canvas_noise_seed_from_env() -> Option<u32> {
    let value = std::env::var_os("BROWSAI_CANVAS_NOISE_SEED")?;
    let s = value.to_str()?;
    s.parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::canvas_noise_seed_from_env;

    /// Helper that runs a closure with BROWSAI_CANVAS_NOISE_SEED set
    /// to a value, then restores the previous state.
    fn with_seed<F: FnOnce()>(value: Option<&str>, f: F) {
        let previous = std::env::var_os("BROWSAI_CANVAS_NOISE_SEED");
        if let Some(v) = value {
            std::env::set_var("BROWSAI_CANVAS_NOISE_SEED", v);
        } else {
            std::env::remove_var("BROWSAI_CANVAS_NOISE_SEED");
        }
        let result = f();
        match previous {
            Some(v) => std::env::set_var("BROWSAI_CANVAS_NOISE_SEED", v),
            None => std::env::remove_var("BROWSAI_CANVAS_NOISE_SEED"),
        }
        result
    }

    #[test]
    fn canvas_noise_seed_returns_none_when_env_var_unset() {
        with_seed(None, || {
            assert_eq!(canvas_noise_seed_from_env(), None);
        });
    }

    #[test]
    fn canvas_noise_seed_parses_valid_integer() {
        with_seed(Some("12345"), || {
            assert_eq!(canvas_noise_seed_from_env(), Some(12345));
        });
    }

    #[test]
    fn canvas_noise_seed_returns_none_for_garbage_value() {
        with_seed(Some("not-a-number"), || {
            assert_eq!(canvas_noise_seed_from_env(), None);
        });
    }

    #[test]
    fn canvas_noise_seed_does_not_perturb_other_env_state() {
        with_seed(Some("42"), || {
            canvas_noise_seed_from_env();
        });
        // After the closure, the variable should be removed.
        assert!(std::env::var_os("BROWSAI_CANVAS_NOISE_SEED").is_none());
    }
}
mod peniko_conversions;
#[cfg(feature = "vello")]
mod vello_backend;
mod vello_cpu_backend;
