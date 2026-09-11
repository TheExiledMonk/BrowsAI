# Benchmark suite

Benchmarks are grouped by `compatibility/`, `memory/`, `concurrency/`,
`latency/`, and `tokens/`. Each run should record JSON containing the commit,
engine capability matrix, workload, sample count, median, p95, and
memory/token counters. Results are comparable only with the same deterministic
headless options and fixture revision.

Required workloads include startup, navigation, DOM observation, layout,
semantic compilation, snapshots, diffs, native input, IPC, storage,
concurrency, recovery, snapshot/query/diff size, provenance overhead,
pagination, and end-to-end agent task cost.

Validate a result artifact with:

```sh
python3 benchmarks/validate.py benchmarks/example-result.json
```

Run the deterministic workload registry with `python3 benchmarks/run.py
--output-dir benchmarks/results`. Use `--check-only` in CI to validate the
registry without creating benchmark artifacts; each measured result is
compatible with `benchmarks/validate.py` and includes bounded token estimates.

The validator enforces the token schema plus reproducibility metadata and
non-negative sample/statistic constraints.
