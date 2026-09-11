"""Transport-neutral client for the BrowsAI agent protocol."""
from dataclasses import dataclass
from typing import Any, Awaitable, Protocol

PROTOCOL_VERSION = 1

@dataclass(frozen=True)
class AgentRequest:
    protocol_version: int
    request_id: str
    agent_id: str
    session_id: str
    operation: Any

@dataclass(frozen=True)
class AgentResponse:
    protocol_version: int
    request_id: str
    result: Any

@dataclass(frozen=True)
class PageSnapshot:
    id: int
    url: str
    tree: Any
    focused_node: str | None
    scroll_x: float
    scroll_y: float
    pending_network: int
    storage_generation: int
    semantic_generation: int

@dataclass(frozen=True)
class Capability:
    name: str
    enabled: bool
    reason: str | None = None

@dataclass(frozen=True)
class Confirmation:
    id: str
    state: str
    summary: str
    expires_at_tick: int | None = None

@dataclass(frozen=True)
class AgentProtocolError:
    code: str
    message: str
    request_id: str | None = None
    details: Any = None

class Transport(Protocol):
    def request(self, method: str, params: dict[str, Any] | None = None) -> Any: ...

class AsyncTransport(Protocol):
    def request(self, method: str, params: dict[str, Any] | None = None) -> Awaitable[Any]: ...

@dataclass(frozen=True)
class Query:
    role: str | None = None
    semantic_role: str | None = None
    application_type: str | None = None
    name: str | None = None
    name_contains: str | None = None
    identity_key: str | None = None
    origin: str | None = None
    visible: bool | None = None
    enabled: bool | None = None
    focused: bool | None = None
    selected: bool | None = None
    geometry: dict[str, float] | None = None
    relationship_kind: str | None = None
    relationship_target: str | None = None

    def to_payload(self) -> dict[str, Any]:
        return {
            "role": self.role,
            "semanticRole": self.semantic_role,
            "applicationType": self.application_type,
            "name": self.name,
            "nameContains": self.name_contains,
            "identityKey": self.identity_key,
            "origin": self.origin,
            "visible": self.visible,
            "enabled": self.enabled,
            "focused": self.focused,
            "selected": self.selected,
            "geometry": self.geometry,
            "relationshipKind": self.relationship_kind,
            "relationshipTarget": self.relationship_target,
        }

@dataclass(frozen=True)
class QueryShape:
    max_results: int | None = None
    max_tokens: int | None = None
    include_value: bool = True
    include_provenance: bool = True
    include_geometry: bool = True

    def to_payload(self) -> dict[str, Any]:
        return {
            "maxResults": self.max_results,
            "maxTokens": self.max_tokens,
            "includeValue": self.include_value,
            "includeProvenance": self.include_provenance,
            "includeGeometry": self.include_geometry,
        }

class AgentClient:
    def __init__(self, transport: Transport) -> None: self._transport = transport
    def query(self, query: Query) -> Any: return self._transport.request("page.query", {"query": query.to_payload()})
    def query_page(self, query: Query, offset: int = 0, limit: int = 100) -> Any:
        return self._transport.request("page.queryPage", {"query": query.to_payload(), "offset": offset, "limit": limit})
    def snapshot(self) -> Any: return self._transport.request("page.snapshot")
    def diff(self, snapshot_id: int) -> Any: return self._transport.request("page.diff", {"snapshotId": snapshot_id})
    def capabilities(self) -> Any: return self._transport.request("agent.capabilities")
    def confirm(self, confirmation_id: str, decision: str) -> Any:
        return self._transport.request("agent.confirm", {"id": confirmation_id, "decision": decision})
    def get(self, node_id: str) -> Any: return self._transport.request("page.get", {"nodeId": node_id})
    def explain(self, node_id: str) -> Any: return self._transport.request("page.explain", {"nodeId": node_id})
    def navigate(self, url: str) -> Any: return self._transport.request("page.navigate", {"url": url})
    def action(self, target: str, action: str, parameters: dict[str, Any] | None = None) -> Any:
        return self._transport.request("page.action", {"target": target, "action": action, "parameters": parameters or {}})
    def subscribe(self, stream: str) -> Any: return self._transport.request("page.subscribe", {"stream": stream})

class AsyncAgentClient:
    """Async parity client using the same wire methods as :class:`AgentClient`."""
    def __init__(self, transport: AsyncTransport) -> None: self._transport = transport
    async def query(self, query: Query) -> Any:
        return await self._transport.request("page.query", {"query": query.to_payload()})
    async def query_page(self, query: Query, offset: int = 0, limit: int = 100) -> Any:
        return await self._transport.request("page.queryPage", {"query": query.to_payload(), "offset": offset, "limit": limit})
    async def snapshot(self) -> Any: return await self._transport.request("page.snapshot")
    async def diff(self, snapshot_id: int) -> Any:
        return await self._transport.request("page.diff", {"snapshotId": snapshot_id})
    async def capabilities(self) -> Any: return await self._transport.request("agent.capabilities")
    async def confirm(self, confirmation_id: str, decision: str) -> Any:
        return await self._transport.request("agent.confirm", {"id": confirmation_id, "decision": decision})
    async def get(self, node_id: str) -> Any:
        return await self._transport.request("page.get", {"nodeId": node_id})
    async def explain(self, node_id: str) -> Any:
        return await self._transport.request("page.explain", {"nodeId": node_id})
    async def navigate(self, url: str) -> Any:
        return await self._transport.request("page.navigate", {"url": url})
    async def action(self, target: str, action: str, parameters: dict[str, Any] | None = None) -> Any:
        return await self._transport.request("page.action", {"target": target, "action": action, "parameters": parameters or {}})
    async def subscribe(self, stream: str) -> Any:
        return await self._transport.request("page.subscribe", {"stream": stream})

class BrowsAI:
    """High-level synchronous protocol client matching the public launch/open flow."""
    @staticmethod
    def launch(transport: Transport, profile: str | None = None, workspace: str | None = None,
               headless: bool = True) -> AgentClient:
        options = {"profile": profile, "workspace": workspace, "headless": headless}
        transport.request("browser.launch", options)
        return AgentClient(transport)

class AsyncBrowsAI:
    """Async counterpart to :class:`BrowsAI`."""
    @staticmethod
    async def launch(transport: AsyncTransport, profile: str | None = None,
                     workspace: str | None = None, headless: bool = True) -> AsyncAgentClient:
        options = {"profile": profile, "workspace": workspace, "headless": headless}
        await transport.request("browser.launch", options)
        return AsyncAgentClient(transport)
