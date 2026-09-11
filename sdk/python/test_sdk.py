import asyncio
import unittest

from browsai_sdk import AgentClient, AsyncAgentClient, BrowsAI, Query


class RecordingTransport:
    def __init__(self):
        self.calls = []

    def request(self, method, params=None):
        self.calls.append((method, params))
        return None


class AsyncRecordingTransport:
    def __init__(self):
        self.calls = []

    async def request(self, method, params=None):
        self.calls.append((method, params))
        return None


class SdkTests(unittest.TestCase):
    def test_query_uses_protocol_camel_case(self):
        transport = RecordingTransport()
        AgentClient(transport).query(Query(semantic_role="SubmitAction", identity_key="button:save", origin="https://example.test", focused=True,
                                           geometry={"minWidth": 80}, relationship_kind="labels", relationship_target="dom:1"))
        self.assertEqual(
            transport.calls[0],
            (
                "page.query",
                {"query": {"role": None, "semanticRole": "SubmitAction", "applicationType": None,
                           "name": None, "nameContains": None, "identityKey": "button:save", "origin": "https://example.test", "visible": None, "enabled": None,
                           "focused": True, "selected": None, "geometry": {"minWidth": 80},
                           "relationshipKind": "labels", "relationshipTarget": "dom:1"}},
            ),
        )

    def test_extended_operations_use_stable_wire_methods(self):
        transport = RecordingTransport()
        client = AgentClient(transport)
        client.snapshot()
        client.diff(7)
        client.capabilities()
        client.confirm("c1", "approve")
        self.assertEqual([call[0] for call in transport.calls], [
            "page.snapshot", "page.diff", "agent.capabilities", "agent.confirm"
        ])
        self.assertEqual(transport.calls[1][1], {"snapshotId": 7})
        self.assertEqual(transport.calls[3][1], {"id": "c1", "decision": "approve"})

    def test_high_level_launch_uses_transport_and_returns_client(self):
        transport = RecordingTransport()
        client = BrowsAI.launch(transport, profile="work", headless=True)
        self.assertIsInstance(client, AgentClient)
        self.assertEqual(transport.calls[0], ("browser.launch", {
            "profile": "work", "workspace": None, "headless": True,
        }))

    def test_async_client_matches_get_and_explain_wire_methods(self):
        transport = AsyncRecordingTransport()

        async def exercise():
            client = AsyncAgentClient(transport)
            await client.get("dom:1")
            await client.explain("dom:1")

        asyncio.run(exercise())
        self.assertEqual(transport.calls, [
            ("page.get", {"nodeId": "dom:1"}),
            ("page.explain", {"nodeId": "dom:1"}),
        ])


if __name__ == "__main__":
    unittest.main()
