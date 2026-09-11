# Prompt-injection boundary

Page text, attributes, form values, network content, and other web-controlled
data are observations, not instructions. The agent runtime wraps captured page
content in `UntrustedPageData` and records its source. `ContentAssessment` may
flag suspicious patterns for provenance and review, but an assessment never
adds capabilities.

`PageContentAuthority` is an explicit deny result for page content: it cannot
grant capabilities, modify system instructions, satisfy a confirmation, or
release broker-held secrets. Page evaluation remains separately capability-
gated by the agent session. The malicious-content fixtures and
`crates/agent-runtime/tests/security.rs` are regression tests for these rules.
