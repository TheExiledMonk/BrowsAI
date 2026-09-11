# Google/high-usage corpus campaign

The resumable campaign is driven by `scripts/run_google_corpus.py`.

```text
python3 scripts/run_google_corpus.py seed-ranked \
  --target 2000 \
  --corpus check-sites/google-2000.csv \
  --ranked-zip /tmp/umbrella-top-1m.csv.zip

python3 scripts/run_google_corpus.py exercise \
  --corpus check-sites/google-2000.csv \
  --state check-sites/google-2000-state.json
```

Each site is isolated in its own real Servo CLI process. The exercise performs
navigation, Agent Render Tree projection, native link clicks, and up to three
native button/checkbox/radio probes. A timeout or non-optional runtime error is
persisted and stops the campaign; rerunning resumes from the failed site after
the defect is fixed.

Current evidence: 29 sites passed. The first unresolved site is
`amazonaws.com`, which redirects to `aws.amazon.com` and does not return from
Servo's load event loop within the 60-second process budget. The failure is
preserved in `check-sites/google-2000-state.json`.

Google discovery is supported with `live-search <query> --discover`, but the
Google session began returning `/sorry/` rate-limit pages during bulk discovery.
Those responses are recorded as discovery retries rather than being treated as
an empty corpus.
