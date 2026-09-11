"""Public compatibility import for the BrowsAI Python SDK."""

from browsai_sdk import (  # noqa: F401
    AgentClient,
    AgentProtocolError,
    AgentRequest,
    AgentResponse,
    AsyncAgentClient,
    AsyncBrowsAI,
    BrowsAI,
    Capability,
    Confirmation,
    PageSnapshot,
    Query,
)

__all__ = [
    "AgentClient", "AgentProtocolError", "AgentRequest", "AgentResponse",
    "AsyncAgentClient", "AsyncBrowsAI", "BrowsAI", "Capability",
    "Confirmation", "PageSnapshot", "Query",
]
