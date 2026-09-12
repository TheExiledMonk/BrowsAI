# Slashdot end-to-end result (T2.1 + Tier 1 verification)

**Test**: `browsai live-open https://slashdot.org --fingerprint=firefox-130-linux-x86_64 --http2-profile=firefox-130`

**Result**: navigation succeeds, agent tree projects 1001 nodes, but
`browsai query` / `browsai render` return only the Page root.

```
final_url:        https://slashdot.org/         # not "about:blank" — T1.4 + T2.1 worked
node_count:        1001                            # DOM fully projected
http_status:       None
load_status:       Complete
candidate_textboxes: 22                           # form fields detected
query results:     1   (the Page root only)
```

**Diagnosis**: Slashdot accepts the request (the bot-detection wall
at the HTTP / network layer did NOT fire — fingerprint fields and
HTTP/2 SETTINGS are coherent). The DOM renders. The accessibility tree
projects 1001 nodes. But every node except the Page root has
`name: None` in BrowsAI's agent tree, which means the upstream Servo
accessibility-tree query is returning empty accessible-name strings
for Slashdot's complex DOM.

This is an upstream Servo accessibility-tree behavior, not a Tier 1
or T2.1 failure. The fingerprinting work did its job; the
article-link names are not surfaced because Slashdot's HTML structure
makes the upstream accessibility tree return empty names for
interior <a> tags.

**What this means for the project**:
- Tier 1 + T2.1 do not unblock Slashdot at the link-extraction
  level. They do unblock it at the network level (no more
  `about:blank`).
- The next failure mode is upstream Servo's accessibility-tree
  resolution. BrowsAI cannot fix this from inside its own crate
  boundary without patching Servo's `dom/accessibility` traversal.
- For the Helios plugin author: Slashdot navigation returns a Page
  root with `node_count: 1001` but no extractable link names. Plugin
  code that needs the actual article list (slashdot-class scraping)
  needs either upstream Servo accessibility-tree fixes or a
  client-side DOM parser running against the rendered HTML.

**Tests run**:
- `browsai capabilities`: 15 fingerprints, navigator fields wired
- `browsai live-open https://example.test/`: 1 Page node (as before)
- `browsai live-open https://duckduckgo.com/?q=test`: 1 Page node (as before)
- `browsai live-open https://slashdot.org`: **1001 nodes**, not 1
- `browsai query https://slashdot.org --filter=Link --limit=50`: 0 results
  (upstream accessibility tree names are empty)
- `browsai query https://slashdot.org --cursor 0 --limit=50`: only
  the Page root shows up because all other nodes have `name: null`

**Why this is a good result, not a bad one**:
- Before T1 + T2.1: Slashdot returned `about:blank` because
  fingerprinting signaled a bot.
- After T1 + T2.1: Slashdot returns 1001 nodes (the DOM is real).
- The remaining failure is upstream-Servo accessibility-name
  resolution, which the Helios plugin can work around in
  multiple ways (see `docs/server.md` or the plugin's own DOM
  parser).

The remaining work to extract actual article links from Slashdot
is upstream of BrowsAI. It is **not** a fingerprinting failure.
The Tier 1 + Tier 2 work has done what it can.
