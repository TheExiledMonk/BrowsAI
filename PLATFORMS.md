# Platform and feature policy

Linux is the first host target and the required CI target. The workspace keeps
engine-neutral APIs so macOS and Windows adapters can follow without changing
agent, provenance, broker, or security contracts.

Feature boundaries are explicit: `engine-api` is always available;
`engine-servo` owns the optional Servo runtime boundary; headless mode is
deterministic and no-raster; desktop rendering is optional. Unsupported engine
features are reported through capability discovery and normalized as unsupported
instead of being guessed.
