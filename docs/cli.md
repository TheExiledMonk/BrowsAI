# CLI reference

The `browsai` binary supports:

- `version` — prints the product and protocol versions;
- `capabilities` — prints adapter feature discovery;
- `headless <url>`, `open <url>`, or `navigate <url>` — performs deterministic no-raster navigation;
- `query <url>` or `render <url>` — renders the Agent Render Tree as JSON. Add
  `--cursor N --limit N` for a bounded page with `truncated` and
  `next_cursor`; safety caps reject excessively large requests.
- `live-open <url>` and `live-search <query>` — exercise the real browser
  runtime. Their link/control/textbox projections accept bounded cursor and
  limit options and return matching truncation metadata.
- `logs <log.json>` — validates a secret-safe structured-log export;
- `audit <journal.json>` — validates an integrity-chained audit journal and reports its record count;
- `replay <journal.json>` — validates and replays an audit journal without mutating it;
- `benchmark <result.json>` — validates required benchmark identity and sample fields;
- `recovery <checkpoint.json>` — validates and prints a persisted recovery checkpoint.

Invalid commands or URLs exit with status `2`. The CLI uses the same
engine-neutral contracts as library callers and does not expose browser
automation shortcuts. Audit and recovery inspection are read-only and reject
corrupt or unsupported persisted data.
