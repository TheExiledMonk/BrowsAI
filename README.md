# BrowsAI

BrowsAI is an AI-native browser architecture. Web content executes in a real browser runtime; agents consume a first-class Agent Render Tree and issue semantic intent that resolves through native browser input.

The repository is currently bootstrapping the engine-neutral contracts and the smallest vertical slice of browser-state-to-agent-state infrastructure. Servo integration belongs behind `engine-api`; it is intentionally not scattered through the core crates.

## Development

```sh
cargo test --workspace
cargo fmt --all -- --check
```

See [ARCHITECTURE.md](ARCHITECTURE.md), [SECURITY.md](SECURITY.md),
[docs/README.md](docs/README.md), [docs/browser-fidelity.md](docs/browser-fidelity.md),
[docs/challenge-handling.md](docs/challenge-handling.md),
[docs/profile-schema.md](docs/profile-schema.md),
[BROWSER_FIDELITY_TODO.md](BROWSER_FIDELITY_TODO.md), and [TODO.md](TODO.md)
for the system design, operational guides, and implementation plan.
