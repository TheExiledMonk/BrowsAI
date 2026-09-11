# Human takeover

Human takeover is an explicit ownership transition. An agent requests a
handoff with a reason and timeout; the human may accept, pause, resume, or
complete it. Expired and completed sessions reject later transitions.

The runtime records every transition for audit correlation and exposes a
redaction helper for handoff payloads. The desktop shell represents the active
handoff as a `Takeover` surface, while the agent remains unable to treat page
content as authority or confirmation.
