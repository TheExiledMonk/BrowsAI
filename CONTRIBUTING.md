# Contributing

Thank you for your interest in contributing to BrowsAI. This document
describes how to participate and the licensing terms for code you submit.

## How to participate

### Reporting bugs

Open an issue in the project's public issue tracker. Include:

- The BrowsAI version (commit SHA) you observed the bug on;
- The command line or host-application entry point you ran;
- A minimal reproduction;
- The expected and actual behaviour;
- The platform (OS, CPU architecture, display state).

If the bug involves a third-party service (Cloudflare, hCaptcha, a
specific website), include the URL and a description of the page state.

### Suggesting features

Open an issue with:

- The use case the feature supports;
- The proposed high-level behaviour;
- The relevant crates and traits that would change;
- Any backward-compatibility concerns.

### Submitting code

1. Fork the repository.
2. Create a topic branch from `main`.
3. Implement the change. Tests are required for new behaviour and bug
   fixes; the workspace `cargo test --workspace` must pass.
4. Run `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`
   locally.
5. Open a pull request against `main`. The pull request description
   should explain the change, link any relevant issue, and list the
   crates that were touched.
6. A reviewer will either approve, request changes, or close the pull
   request.

For larger changes, please open an issue first to discuss the design
before investing time in implementation.

### Code style

- Follow `rustfmt` defaults.
- Follow `clippy` defaults; warnings are denied in CI.
- Prefer explicit error types over `anyhow` for library code.
- Public API changes that break backward compatibility require a
  release-notes entry.

## Contribution licensing

By submitting a contribution (a pull request, a patch, a code sample,
documentation, or any other contribution) to this project, You agree to
the following terms.

### Grant of rights

You retain copyright in Your contribution. You grant the BrowsAI
copyright holders a perpetual, worldwide, non-exclusive, royalty-free,
irrevocable licence to:

- use, reproduce, modify, adapt, and create derivative works of Your
  contribution;
- publicly perform, publicly display, distribute, sublicense, and
  otherwise exploit Your contribution; and
- incorporate Your contribution into the Work, into any derivative work
  of the Work, and into any commercial product or service offered
  under the BrowsAI commercial licence.

### Distribution under multiple licences

The BrowsAI copyright holders may distribute Your contribution, and
any derivative work that includes Your contribution, under both:

- the BrowsAI public-use licence set out in `LICENSE`; and
- any BrowsAI commercial licence offered under
  `LICENSE-COMMERCIAL.md`.

You acknowledge that the BrowsAI public-use licence restricts
External Commercial Use without a separate commercial licence, and that
the BrowsAI copyright holders may grant separate commercial licences
that include Your contribution.

### Originality

You represent that:

- Your contribution is Your own original work, or You have sufficient
  rights to submit it under these terms;
- Your contribution does not knowingly infringe any third party's
  copyright, patent, trademark, trade secret, or other intellectual
  property right;
- You have disclosed any prior obligations (employer agreements,
  research-grant terms, prior contributions) that might affect Your
  ability to grant the rights above.

If Your contribution is based on someone else's work, identify that
work in the pull-request description and confirm that You have the
rights to grant the licence above.

### No expectation of payment

Submitting a contribution does not entitle You to any payment,
royalties, or other compensation. The BrowsAI copyright holders may, at
their sole discretion, acknowledge contributions in release notes or a
`CONTRIBUTORS.md` file.

## Reporting security issues

See `SECURITY.md`-equivalent guidance in the project documentation
once it is restored. Until then, please report security issues through
the public issue tracker with a clear `[security]` prefix so a
maintainer can mark the issue private.

## Code of conduct

Be respectful. Critique code, not people. Assume good faith. Report
behaviour that violates this code of conduct to the project owner
through a private channel.