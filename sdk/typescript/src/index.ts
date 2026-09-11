export interface Query {
  role?: string;
  semanticRole?: string;
  applicationType?: string;
  name?: string;
  nameContains?: string;
  identityKey?: string;
  origin?: string;
  visible?: boolean;
  enabled?: boolean;
  focused?: boolean;
  selected?: boolean;
  geometry?: {
    minX?: number;
    maxX?: number;
    minY?: number;
    maxY?: number;
    minWidth?: number;
    minHeight?: number;
  };
  relationshipKind?: string;
  relationshipTarget?: string;
}

export interface PageSnapshot {
  id: number;
  url: string;
  tree: unknown;
  focusedNode?: string;
  scrollX: number;
  scrollY: number;
  pendingNetwork: number;
  storageGeneration: number;
  semanticGeneration: number;
}

export interface DiffOperation {
  type: "add" | "remove" | "replace";
  path: string;
  value?: unknown;
  old?: unknown;
  new?: unknown;
}

export interface PageDiff {
  operations: readonly DiffOperation[];
}

export interface Capability {
  name: string;
  enabled: boolean;
  reason?: string;
}

export interface Confirmation {
  id: string;
  state: "pending" | "approved" | "denied" | "expired" | "escalated" | string;
  summary: string;
  expiresAtTick?: number;
}

export interface ChangeEvent {
  type: string;
  [field: string]: unknown;
}

export interface AgentProtocolError {
  code: string;
  message: string;
  requestId?: string;
  details?: unknown;
}

export interface QueryResult {
  nodeId: string;
  name?: string;
  role: string;
  semanticRole?: string;
  value?: unknown;
  confidence: number;
  provenance?: readonly unknown[];
  geometry?: { x: number; y: number; width: number; height: number };
  origin?: string;
}

export interface QueryPage {
  results: readonly QueryResult[];
  offset: number;
  nextOffset?: number;
  total: number;
}

export const PROTOCOL_VERSION = 1;

export interface AgentRequest {
  protocolVersion: number;
  requestId: string;
  agentId: string;
  sessionId: string;
  operation: unknown;
}

export interface AgentResponse<T = unknown> {
  protocolVersion: number;
  requestId: string;
  result: T;
}

export interface PageClient {
  query(query: Query): Promise<readonly QueryResult[]>;
  queryPage(query: Query, offset?: number, limit?: number): Promise<QueryPage>;
  snapshot(): Promise<PageSnapshot>;
  diff(snapshotId: number): Promise<PageDiff>;
  capabilities(): Promise<readonly Capability[]>;
  confirm(id: string, decision: "approve" | "deny" | "escalate"): Promise<Confirmation>;
  get(nodeId: string): Promise<QueryResult | undefined>;
  explain(nodeId: string): Promise<unknown | undefined>;
  navigate(url: string): Promise<unknown>;
  action(target: string, action: string, parameters?: unknown): Promise<unknown>;
  subscribe(stream: string): Promise<unknown>;
}

export interface AgentTransport {
  request<T>(method: string, params?: unknown): Promise<T>;
}

export interface LaunchOptions {
  profile?: string;
  workspace?: string;
  headless?: boolean;
}

/** Page content is transported as data; this client never evaluates it as instructions. */
export class AgentClient implements PageClient {
  constructor(private readonly transport: AgentTransport) {}
  query(query: Query): Promise<readonly QueryResult[]> { return this.transport.request("page.query", { query }); }
  queryPage(query: Query, offset = 0, limit = 100): Promise<QueryPage> {
    return this.transport.request("page.queryPage", { query, offset, limit });
  }
  snapshot(): Promise<PageSnapshot> { return this.transport.request("page.snapshot"); }
  diff(snapshotId: number): Promise<PageDiff> {
    return this.transport.request("page.diff", { snapshotId });
  }
  capabilities(): Promise<readonly Capability[]> {
    return this.transport.request("agent.capabilities");
  }
  confirm(id: string, decision: "approve" | "deny" | "escalate"): Promise<Confirmation> {
    return this.transport.request("agent.confirm", { id, decision });
  }
  get(nodeId: string): Promise<QueryResult | undefined> { return this.transport.request("page.get", { nodeId }); }
  explain(nodeId: string): Promise<unknown | undefined> { return this.transport.request("page.explain", { nodeId }); }
  navigate(url: string): Promise<unknown> { return this.transport.request("page.navigate", { url }); }
  action(target: string, action: string, parameters: unknown = {}): Promise<unknown> {
    return this.transport.request("page.action", { target, action, parameters });
  }
  subscribe(stream: string): Promise<unknown> { return this.transport.request("page.subscribe", { stream }); }
}

/** High-level protocol client matching the public BrowsAI launch/open flow. */
export class BrowserClient {
  constructor(
    private readonly transport: AgentTransport,
    readonly options: LaunchOptions = {},
  ) {}

  async open(url: string): Promise<AgentClient> {
    await this.transport.request("page.navigate", { url });
    return new AgentClient(this.transport);
  }

  capabilities(): Promise<readonly Capability[]> {
    return this.transport.request("agent.capabilities");
  }
}

export class BrowsAI {
  static async launch(
    transport: AgentTransport,
    options: LaunchOptions = {},
  ): Promise<BrowserClient> {
    await transport.request("browser.launch", options);
    return new BrowserClient(transport, options);
  }
}
