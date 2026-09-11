# Agent runtime

Agent JavaScript is represented by `AgentScriptRuntime`, which has a separate
lifecycle from page evaluation. An `AgentScriptRequest` has provenance and a
bounded timeout but no page origin; starting it consumes the
`ExecuteAgentScript` capability and binds the execution to an agent session.

Page JavaScript remains behind the browser engine's
`evaluate_page_script_checked` API and its `ExecutePageScript` capability.
The two paths therefore cannot be confused by a shared page-evaluation call.
Agent output is accounted against the session output quota, and unfinished
executions can be cancelled without exposing their source to page state.
