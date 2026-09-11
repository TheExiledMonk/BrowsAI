# Browser Fidelity

The BrowsAI engine presents a coherent browser identity that is derived from
the active profile, not from ad-hoc strings. This document explains where each
identity value comes from, what overrides are allowed, and which are
explicitly rejected.

## Identity model

```text
ProfileConfig -> ProfileIdentity -> ContextOptions -> ServoRuntime / NetworkStack
                                  \-> build_navigator_identity_script
                                  \-> NetworkStack::apply_profile_headers

Fingerprint (catalog) -> ProfileManager::apply_fingerprint -> ProfileConfig
                                                  \-> ProfileIdentity
```

Every identity-bearing value traces back to a single `ProfileIdentity`:

| Field             | Source                                       | Used for                                                       |
| ----------------- | -------------------------------------------- | -------------------------------------------------------------- |
| `user_agent`      | `ProfileConfig.user_agent` (default Chrome)   | HTTP `User-Agent`, `navigator.userAgent`, `navigator.appVersion` |
| `locale`          | `ProfileConfig.locale`                       | Servo's `intl_locale_override`, `navigator.language`           |
| `timezone`        | `ProfileConfig.timezone`                     | process `TZ`, `Intl.DateTimeFormat`, `Date`                    |
| `viewport`        | `ProfileConfig.viewport`                     | Servo `SoftwareRenderingContext`, `screen.{width, height, ...}` |
| `platform`        | `ProfileConfig.platform`                     | `navigator.platform`, `Sec-CH-UA-Platform`, `User-Agent` family check |
| `brands`          | `ProfileConfig.brands`                       | `Sec-CH-UA`, `navigator.userAgentData.brands`                  |
| `accept_language` | `ProfileConfig.accept_language`              | HTTP `Accept-Language`, `Sec-CH-UA-Mobile` derivation         |

`ProfileIdentity` is constructed once per `ContextOptions`. The deterministic
headless backend uses it directly; the real Servo runtime reads it at
`ServoRuntime::new` and again at `ServoRuntimePage::create_page_with_identity`,
which installs `build_navigator_identity_script` so the per-page values stay
in sync even if a different page within the same runtime overrides the
identity.

## Fingerprint catalog

`browsai_fingerprint` bundles the identity-bearing values plus metadata that
the page can observe (`webgl_vendor`, `webgl_renderer`, `font_set`,
`canvas_noise_seed`). Each catalog entry is required to be internally
consistent before it can be inserted; `FingerprintCatalog::insert` rejects a
fingerprint whose `User-Agent` family disagrees with its `brands`, whose
platform disagrees with the `User-Agent`'s platform hint, or whose WebGL
vendor disagrees with the family.

The default catalog ships with five vetted fingerprints:

| ID                              | Family | Platform |
| ------------------------------- | ------ | -------- |
| `chrome-140-linux-x86_64`       | Chrome | Linux    |
| `firefox-130-linux-x86_64`      | Firefox| Linux    |
| `safari-17-macos-arm64`         | Safari | macOS    |
| `chrome-android-140-pixel8`     | Chrome | Android  |
| `safari-ios-17-iphone`          | Safari | iOS      |

`ProfileManager::apply_fingerprint(catalog, fingerprint_id)` resolves a
fingerprint, runs `is_consistent`, and applies the resulting `ProfileConfig`
through `try_update_config`, so the same consistency rules apply whether the
profile was built from a `ProfileVariant` or from a catalog fingerprint.

## Coherence checks

`browsai_profiles::check_profile_consistency` rejects a profile whose
`user_agent` advertises one browser family while `brands` claims another:

```text
UA = "Mozilla/5.0 ... Firefox/130.0"
brands = ["Google Chrome";v="140"]
=>  Err(ProfileInconsistency::UserAgentClaimsBrowser { claimed: "firefox", declared: "chrome/safari brands" })
```

It also rejects platform mismatches (UA says Linux, `platform` says Windows).
The check fires inside `ProfileManager::try_update_config`, so the offending
config never reaches the engine.

When `apply_fingerprint` is given a fingerprint whose `is_consistent`
returns an `Err`, the error is propagated. If a future caller wants to
*surface* the inconsistency rather than reject it, the
`SemanticIr::with_profile_inconsistencies` method emits a
`SemanticRole::ProfileInconsistency` summary node on the agent tree and
`ChangeEvent::ProfileInconsistency` appears on the change stream.

## HTTP header derivation

`NetworkStack::apply_profile_headers` is invoked on every fetch (including
`fetch_worker`). It fills in:

- `User-Agent`
- `Accept-Language`
- `Sec-CH-UA` (only if `brands` is non-empty)
- `Sec-CH-UA-Mobile`
- `Sec-CH-UA-Platform`

…only when the caller has not supplied the header. The caller can replace a
header but cannot delete one — the profile's required identity headers are
always present. (Profiles that intentionally have empty `brands`, like
Firefox or Safari, do not produce a `Sec-CH-UA` header, matching real
browser behavior.)

## Override policy

The `BROWSAI_USER_AGENT`, `BROWSAI_LOCALE`, and `BROWSAI_TIMEZONE`
environment variables remain as fallbacks but emit a `deprecation` warning
when used. They will be removed once the profile path is universal.

## Out of scope

This document and the surrounding code do not implement:

- **TLS fingerprint spoofing (JA3/JA4).** That is an engine-level concern;
  the Servo embedder must produce a real Chrome-compatible TLS fingerprint.
  The `browsai-fingerprint` crate does not touch TLS.
- **Residential proxy rotation or IP-reputation management.** The
  `NetworkStack::proxy` field exists for legitimate network egress, not for
  evading IP-based blocking.
- **Cookie or fingerprint *cross-session* persistence beyond what
  `ProfileManager` already stores.** Each profile owns its own state.

If you need any of those, this is the wrong layer — the answer is upstream
network infrastructure or a different tool, not a BrowsAI feature.

The project does ship a consent-gated humanized mouse trajectory in
`browsai-input` (see `docs/challenge-handling.md`); it is opt-in per
challenge click via `AttemptCh*` methods and is never dispatched unless a
`TakeoverSession` with `SolveChallenge` (or `CredentialSolve`) is active.