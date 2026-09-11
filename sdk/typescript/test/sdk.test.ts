import {
  AgentClient,
  AgentTransport,
  BrowsAI,
  PROTOCOL_VERSION,
  Query,
} from "../src/index.js";

const transport: AgentTransport = {
  async request<T>(_method: string, _params?: unknown): Promise<T> {
    return undefined as T;
  },
};

async function protocolSurfaceCompiles(): Promise<void> {
  const client = await BrowsAI.launch(transport, { headless: true });
  const page: AgentClient = await client.open("https://example.test");
  const query: Query = { origin: "https://example.test", focused: true };
  await page.queryPage(query, 0, 10);
  await page.get("dom:1");
  await page.explain("dom:1");
  await page.action("dom:1", "click");
  const protocolVersion: number = PROTOCOL_VERSION;
  void protocolVersion;
}

void protocolSurfaceCompiles;
