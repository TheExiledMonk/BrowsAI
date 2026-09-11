# Packaging

`manifest.json` is the source of truth for the buildable headless executable
and the optional desktop-shell library. The headless artifact is built with
the workspace toolchain; runtime fixtures and documentation are copied as
explicit assets rather than discovered implicitly at runtime.

Linux is the supported host target. macOS and Windows entries are maintained
as an explicit roadmap until their process sandbox and desktop integrations
are implemented.

Build the headless artifact with:

```text
packaging/build.sh headless dist
```

The script never embeds credentials or profile data. Profiles, cookies,
storage, and recovery checkpoints remain runtime-owned state.
