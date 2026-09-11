# Licensing

BrowsAI uses a source-available, dual-licensing model. The repository
ships two files:

- `LICENSE` — the public-licence grant (free for Internal Use)
- `LICENSE-COMMERCIAL.md` — pointer to commercial licensing (required
  for External Commercial Use)

## The distinction in one line

> Internal Use is permitted by the public licence. External Commercial
> Use is not.

## Definitions

The full definitions are in `LICENSE`. The two most important terms are:

**Internal Use** — use by You, Your employees, contractors, and
authorised agents solely for Your own internal business or organisational
operations. Includes internal research, internal automation, internal
quality assurance, internal evaluation, and similar activities that are
not directed at providing the Work, or a materially-derived function, to
any third party as part of a paid or otherwise commercially-monetised
offering.

**External Commercial Use** — anything that is not Internal Use. See
`LICENSE` section 1 for the exhaustive list; the common cases are
embedding in a commercial product, providing the Work as a hosted
service, redistributing as part of a paid bundle, or using the Work as
a material customer-facing component of a paid offering.

## Allowed under the public licence (without a commercial licence)

- Personal automation and experimentation
- Academic and industrial research
- University and classroom use
- Internal company browser automation
- Internal company AI agents running against your own infrastructure
- Internal quality assurance and browser compatibility testing
- Internal proof-of-concept development
- Employee productivity tooling
- Internal data collection, where legally permitted
- Evaluation prior to commercial adoption
- Forks and modified versions, provided they are used only for Internal
  Use and the licensing terms are preserved
- Sharing modifications back upstream as pull requests

## Requires a separate commercial licence

- Selling BrowsAI itself
- Embedding BrowsAI into commercial software sold to customers
- Offering BrowsAI-powered browser automation as a SaaS or managed
  service
- Providing BrowsAI as a paid hosted API
- Bundling BrowsAI into an enterprise product
- Operating BrowsAI as a material part of a customer-facing paid
  AI-agent service
- Redistributing a modified BrowsAI as part of a commercial product
- Any other External Commercial Use, as defined in `LICENSE`

## Consulting edge case

A consulting firm or agency may use BrowsAI internally while performing
work for its own clients. A commercial licence is required if:

- BrowsAI itself is delivered to the client;
- the client receives a BrowsAI-powered hosted service;
- the consultant resells BrowsAI functionality; or
- BrowsAI becomes a material customer-facing component of the
  delivered commercial product or service.

If the work is purely advisory (the consultant's output is a written
report or recommendation, not a deployed BrowsAI instance), Internal
Use covers it.

## Cloud and hosted use

A company may run BrowsAI on its own infrastructure solely for its own
internal operations without a commercial licence.

A company that runs BrowsAI on its own infrastructure (or any
third-party infrastructure) and exposes its functionality to paying
customers or other third parties as a service needs a commercial
licence.

## Modified versions

Modified versions may be shared under the same public licence, provided
all required notices, copyright statements, and the full licence text
remain intact and the commercial-use restriction is preserved. A
modified version may not be distributed under terms that remove the
commercial-use restriction.

## Forks

Public forks are allowed under the public licence for the same purposes
as the original Work. A fork must preserve:

- copyright notices;
- the licence text;
- required attribution notices; and
- the commercial-use boundary in this licence.

A fork must not remove or weaken the commercial-use restriction and
present the resulting BrowsAI-derived code as unrestricted.

## Contributions

By submitting a contribution, the contributor agrees that the
contribution may be distributed under the BrowsAI public licence and
under any commercial licence offered by the copyright holders.

Contributors retain copyright in their contributions but grant the
copyright holders a sufficient licence to sublicense the contribution
under both the public licence and any commercial licence. See
`CONTRIBUTING.md` for the full contribution terms.

## Dependency licensing

BrowsAI depends on third-party components, including Servo and many
Rust crates. The BrowsAI licence does not apply to those components;
each is governed by its own licence. The complete inventory and
required notices are maintained in `THIRD_PARTY_LICENSES.md`.

In particular:

- Servo is licensed under MPL 2.0. BrowsAI does not relicense Servo;
  downstream users of BrowsAI must comply with Servo's licence terms.
- BrowsAI may not be redistributed without preserving all required
  third-party notices.

## Distribution artefacts

Packaged BrowsAI binaries and packages include:

- `LICENSE`
- `LICENSE-COMMERCIAL.md`
- `THIRD_PARTY_LICENSES.md`
- `README.md`

The release system is expected to verify their inclusion.

## FAQ

### Can I use BrowsAI at work?

Yes, for Internal Use under the public licence.

### Can my company use BrowsAI internally?

Yes. The public licence explicitly permits internal organisational use,
including internal use by commercial companies.

### Can I build internal automation with BrowsAI?

Yes.

### Can I modify BrowsAI internally?

Yes, for Internal Use. Modified versions may be redistributed under
the same public licence provided the commercial-use boundary is
preserved.

### Can I sell software that embeds BrowsAI?

Not under the public licence. A commercial licence is required. See
`LICENSE-COMMERCIAL.md`.

### Can I offer BrowsAI as a SaaS or paid hosted service?

A commercial licence is required.

### Can I fork BrowsAI?

Yes, under the public licence for the same permitted purposes. The
fork must preserve the licence terms and the commercial-use boundary.

### Can I use BrowsAI for research?

Yes.

### Can a university use it?

Yes.

### Can a consultant use it?

Internal consulting use is permitted. Delivering BrowsAI itself, or
a BrowsAI-powered hosted service, to a client as part of a paid
engagement requires a commercial licence.

### Do I need to credit BrowsAI when I use it?

Yes, the public licence requires preservation of the copyright notices
and licence texts accompanying the Work. For Internal Use where the
Work is integrated into Your own systems, keeping the `LICENSE` file
in the source tree and a comment in the build artefact is sufficient.
For distribution beyond Your organisation, see the distribution
artefacts section above.

### What happens if a fork removes the commercial-use restriction?

The fork remains under this licence. Any External Commercial Use of the
fork still requires a commercial licence from the original copyright
holders. The fork may not legally assert that it is unrestricted.

## Legal review status

This licence is published without a formal legal opinion. The wording
is the project owner's expressed intent; it has not been reviewed by a
qualified lawyer. If You need legally-vetted licence text, treat this
document as a statement of intent rather than a binding legal opinion,
and obtain Your own legal advice before relying on it in a
commercial dispute.

The project owner has chosen to publish without external legal review
on the basis that the public-licence grant is permissive (free for
Internal Use) and the restrictions on External Commercial Use are
enforceable on the basis of copyright and contract law in most
jurisdictions. You are responsible for verifying that the terms of
this licence are appropriate for Your use case.