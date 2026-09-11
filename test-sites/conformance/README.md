# Conformance fixture format

Each JSON fixture contains `id`, `api`, `input`, and `expected`. The `api` value
matches `browsai-conformance::WebApi`. Fixture runners record actual output in a
compatibility report and preserve failures as regressions.
