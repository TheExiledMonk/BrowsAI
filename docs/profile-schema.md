# Profile Schema

The `browsai_profiles` crate stores its state as JSON. This document
describes the on-disk schema and the migration rules.

## Schema versions

| Version | Introduced | Notes |
| ------- | ---------- | ----- |
| `1`     | initial    | `user_agent`, `locale`, `timezone`, `homepage`, `extensions`, `history`, `autocomplete`. |
| `2`     | current    | Adds `viewport`, `platform`, `brands`, `accept_language`. v1 JSON loads via `migrate_v1_to_v2`. |

`ProfileManager::SCHEMA_VERSION` is `2`. The `from_json` loader rejects any
schema version greater than `SCHEMA_VERSION`.

## Fields added in v2

- `viewport: { width, height, device_scale_factor }` — virtual viewport for
  Servo, in CSS pixels.
- `platform: string` — value reported via `navigator.platform` and
  `Sec-CH-UA-Platform`. Must be consistent with the `User-Agent`'s platform
  hint.
- `brands: [{ brand, version }]` — `Sec-CH-UA` brand list. Empty for
  Firefox and Safari families.
- `accept_language: string` — HTTP `Accept-Language` value.

These fields are `#[serde(default)]`, so v1 JSON without them loads cleanly;
the migration in `migrate_v1_to_v2` re-applies the defaults to make sure the
saved v2 snapshot is consistent on the next save.

## Variant catalog

`ProfileVariant` provides a single, internally consistent default for five
common browser profiles:

- `ChromeDesktop`
- `FirefoxDesktop`
- `SafariDesktop`
- `ChromeAndroid`
- `SafariIos`

Each variant produces a `ProfileConfig` that passes
`check_profile_consistency`. Callers can construct one with
`ProfileManager::create_with_variant(name, variant)`.

## Consistency rules

`ProfileManager::try_update_config` runs `check_profile_consistency` before
applying the update. The check rejects:

- `UserAgentClaimsBrowser` when the `User-Agent` family does not match any
  declared brand (Chrome UA with Firefox brands, etc.).
- `PlatformMismatch` when the `User-Agent`'s platform hint disagrees with
  `ProfileConfig.platform`.

The browser-identity surface is therefore coherent by construction: it is
impossible to ship a profile whose `navigator.userAgent` and
`navigator.userAgentData.brands` disagree.