# Testing

Controlled pages under `test-sites/` are local and deterministic. Run
`python3 scripts/validate_test_sites.py` before browser or observer tests; the
validator checks that all fixture groups are present, JSON fixtures parse, and
pages do not introduce remote script or resource dependencies.

Rust unit and integration tests are run with:

```text
cargo test --workspace --all-features --no-fail-fast
```

Golden fixtures are executed by the semantic, CSS, layout, and conformance
tests rather than treated as documentation-only samples.
