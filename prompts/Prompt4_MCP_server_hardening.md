# Prompt 4 — MCP Server Hardening

## Your role

You are implementing production improvements to **TileGraphAgent**. This session covers **Project 4, Stages 4.1–4.4** from `plan.md`: making the MCP server production-safe with Neo4j connection pooling, input validation hardening, WebSocket heartbeats, and queryable audit logs.

**This session does NOT cover** Stage 4.5 (LLM integration test) — that is Prompt 6.

## Repository overview

- **MCP server:** `apps/tilegraph-mcp-server/`
- **Key source files:**
  - `src/db/neo4j.ts` — `Neo4jClient`
  - `src/viewer/bridge.ts` — `ViewerBridge`
  - `src/audit/logger.ts` — `AuditLogger`
  - `src/tools/index.ts` — tool registration and dispatch
  - `src/tools/*.ts` — individual tools with Zod input schemas
  - `src/resources/index.ts` — MCP resources
  - `src/index.ts` — server entry point

**Commands:**

```bash
cd apps/tilegraph-mcp-server
npm install
npm run dev      # tsx watch — starts MCP server
npm run build    # tsc → dist/
npm run test     # vitest run
```

**Read all the above files** before making changes. They were scaffolded in the initial project with known production gaps.

---

## Stage 4.1 — Neo4j connection pooling and health check

### Problem

`Neo4jClient.query()` in `src/db/neo4j.ts` opens a new session per query with `this.driver.session()`, which is closed in `finally`. Under concurrent agent tool calls (which can run multiple queries), this creates and destroys sessions rapidly. The neo4j-driver supports session reuse internally, but it also has a maximum pool size — without explicit control, we burn connections.

### What to change in `src/db/neo4j.ts`

**Add session pool tracking and a health check method.** The neo4j-driver's connection pool is managed internally, but we should:

1. Configure explicit pool parameters via driver config
2. Add a `healthCheck()` method
3. Add a configurable query timeout
4. Handle `ServiceUnavailable` errors distinctly

Replace the constructor:

```typescript
import neo4j, { Driver, Session, Config } from "neo4j-driver"
import { NEO4J_CONNECTION_TIMEOUT_MS } from "../config.js"

export class Neo4jClient {
  private driver: Driver
  private database: string

  constructor(config: Neo4jConfig) {
    const driverConfig: Config = {
      maxConnectionPoolSize: 10,
      connectionAcquisitionTimeout: NEO4J_CONNECTION_TIMEOUT_MS,
      connectionTimeout: NEO4J_CONNECTION_TIMEOUT_MS,
    }
    this.driver = neo4j.driver(config.url, neo4j.auth.basic(config.username, config.password), driverConfig)
    this.database = config.database
  }

  async query<T = Record<string, unknown>>(
    cypher: string,
    params: Record<string, unknown> = {},
    timeoutMs = 3000,
  ): Promise<T[]> {
    const session: Session = this.driver.session({
      database: this.database,
      defaultAccessMode: neo4j.session.READ,
    })
    try {
      const result = await Promise.race([
        session.run(cypher, params),
        new Promise<never>((_, reject) => setTimeout(() => reject(new Error("Query timeout")), timeoutMs)),
      ])
      return (result as Awaited<ReturnType<typeof session.run>>).records.map((r) => {
        const obj: Record<string, unknown> = {}
        for (const key of r.keys) {
          const val = r.get(key as string)
          obj[key as string] = neo4j.isInt(val)
            ? val.toNumber()
            : val instanceof neo4j.types.Node
              ? { ...val.properties, _labels: val.labels }
              : val
        }
        return obj as T
      })
    } catch (err: any) {
      if (err.code === "ServiceUnavailable" || err.message?.includes("timeout")) {
        throw Object.assign(new Error("Graph database unavailable"), {
          error_code: "GRAPH_UNAVAILABLE",
          original: err.message,
        })
      }
      throw err
    } finally {
      await session.close()
    }
  }

  async healthCheck(): Promise<{ connected: boolean; latency_ms: number }> {
    const t0 = Date.now()
    try {
      await this.query("RETURN 1 AS ok", {}, 2000)
      return { connected: true, latency_ms: Date.now() - t0 }
    } catch {
      return { connected: false, latency_ms: -1 }
    }
  }

  async close(): Promise<void> {
    await this.driver.close()
  }

  // ... rest of canonical query methods unchanged
}
```

**Create `src/config.ts`:**

```typescript
export const NEO4J_CONNECTION_TIMEOUT_MS = parseInt(process.env.NEO4J_CONNECTION_TIMEOUT_MS ?? "5000")
export const REST_PORT = parseInt(process.env.REST_PORT ?? "9000")
export const VIEWER_WS_PORT = parseInt(process.env.VIEWER_WS_PORT ?? "9001")
export const SPATIAL_INDEX_PATH = process.env.SPATIAL_INDEX_PATH ?? "output/tiles/index/spatial_index.json"
export const AUDIT_LOG_PATH = process.env.AUDIT_LOG_PATH ?? "output/reports/audit.jsonl"
```

**Update `src/index.ts`** to fail fast if Neo4j is unavailable at startup:

```typescript
// After constructing neo4j client, before registering tools:
const health = await neo4j.healthCheck()
if (!health.connected) {
  console.error(`[STARTUP] Neo4j unavailable at ${process.env.NEO4J_URL}. Continuing without graph queries.`)
} else {
  console.error(`[STARTUP] Neo4j connected (${health.latency_ms}ms)`)
}
```

---

## Stage 4.2 — Tool input validation hardening

### Problem

Zod schemas are shallow. A tool like `get_tile_feature_mapping` accepts `object_ids: z.array(z.string())` with no upper bound — a 10,000-element array would send a massive IN clause to Neo4j.

### What to change

**Create `src/schemas/validation.ts`** with shared validators:

```typescript
import { z } from "zod"

// Validated tag: uppercase alphanumeric + hyphens, max 64 chars
export const TagSchema = z
  .string()
  .min(1)
  .max(64)
  .regex(/^[A-Z0-9\-_\.]+$/i, "Tag must contain only alphanumeric, dash, underscore, or dot characters")

// Validated object_id: must match the pipeline format obj_<32 hex chars>
export const ObjectIdSchema = z.string().regex(/^obj_[a-f0-9]{32}$/, "object_id must be in format obj_<32 hex chars>")

// Bounded object_id array for batch operations
export const ObjectIdArraySchema = z.array(ObjectIdSchema).min(1).max(50)

// Radius for spatial queries — positive, max 500m to prevent runaway queries
export const RadiusSchema = z.number().positive().max(500).default(5.0)

// Direction for upstream/downstream
export const DirectionSchema = z.enum(["upstream", "downstream", "both"]).default("both")
```

**Update each tool file** to use the new schemas. Example for `search_object_by_tag.ts`:

```typescript
// Replace:
const InputSchema = z.object({
  tag: z.string().describe("Engineering tag, e.g. P-1001 or LINE-1001"),
})

// With:
import { TagSchema } from "../schemas/validation.js"
const InputSchema = z.object({
  tag: TagSchema.describe("Engineering tag, e.g. P-1001 or LINE-1001"),
})
```

Apply the same pattern to all tools — replace bare `z.string()` with `TagSchema` or `ObjectIdSchema` as appropriate, and replace bare `z.array(z.string())` with `ObjectIdArraySchema`.

**Tools to update** (go through each one):

- `search_object_by_tag.ts` — `tag` field → `TagSchema`
- `get_object_properties.ts` — `object_id` field → `ObjectIdSchema`
- `query_connected_components.ts` — `object_id` → `ObjectIdSchema`
- `query_upstream_downstream.ts` — `object_id` → `ObjectIdSchema`; `direction` → `DirectionSchema`; add `max_hops: z.number().int().min(1).max(10).default(3)`
- `query_objects_in_area.ts` — `area_tag` → `TagSchema`
- `query_nearby_objects.ts` — `object_id` → `ObjectIdSchema`; `radius_m` → `RadiusSchema`
- `get_tile_feature_mapping.ts` — `object_ids` → `ObjectIdArraySchema`
- `highlight_objects_in_viewer.ts` — `object_ids` → `ObjectIdArraySchema`
- `isolate_system_in_viewer.ts` — `object_ids` → `ObjectIdArraySchema`
- `focus_camera_on_objects.ts` — `object_ids` → `ObjectIdArraySchema`
- `create_issue_from_selection.ts` — `object_id` → `ObjectIdSchema`
- `generate_maintenance_context.ts` — `line_tag` → `TagSchema`

**Update error handling in `src/tools/index.ts`** to distinguish Zod validation errors from runtime errors:

```typescript
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params
  const tool = TOOLS.find((t) => t.definition.name === name)
  if (!tool) {
    return { content: [{ type: "text", text: `Unknown tool: ${name}` }], isError: true }
  }

  const t0 = Date.now()
  try {
    const result = await tool.handler(args ?? {}, ctx)
    await ctx.auditLogger.log({
      tool_name: name,
      input: args,
      output_summary: typeof result === "object" ? JSON.stringify(result).slice(0, 200) : String(result),
      duration_ms: Date.now() - t0,
    })
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] }
  } catch (err: any) {
    // Distinguish Zod validation errors
    const isValidationError = err?.name === "ZodError"
    const error_code = isValidationError ? "VALIDATION_ERROR" : (err?.error_code ?? "INTERNAL_ERROR")
    const message = isValidationError
      ? `Invalid input: ${err.errors.map((e: any) => `${e.path.join(".")}: ${e.message}`).join("; ")}`
      : err instanceof Error
        ? err.message
        : String(err)

    await ctx.auditLogger.log({
      tool_name: name,
      input: args,
      output_summary: `${error_code}: ${message.slice(0, 100)}`,
      duration_ms: Date.now() - t0,
      error: error_code,
    })
    return {
      content: [
        {
          type: "text",
          text: JSON.stringify({
            error_code,
            message,
            tool: name,
          }),
        },
      ],
      isError: true,
    }
  }
})
```

---

## Stage 4.3 — WebSocket heartbeat and command queue

### Problem

`ViewerBridge` in `src/viewer/bridge.ts` has no heartbeat. If a viewer tab is closed abruptly (network drop, browser crash), the server never detects it and continues sending commands to dead sockets. Also, a new viewer connection has no way to catch up on commands sent while it was disconnected.

### What to change in `src/viewer/bridge.ts`

Replace the entire implementation:

```typescript
import { WebSocketServer, WebSocket } from "ws"

export type ViewerCommand =
  | { type: "highlight_objects"; object_ids: string[]; color?: string }
  | { type: "isolate_objects"; object_ids: string[] }
  | { type: "focus_camera"; object_ids: string[] }
  | { type: "show_bounding_boxes"; object_ids: string[] }
  | { type: "clear_highlights" }
  | { type: "create_issue_marker"; object_id: string; title: string; severity: string }
  | { type: "ping" }
  | { type: "pong" }

interface ViewerClient {
  id: string
  ws: WebSocket
  connectedAt: Date
  lastPongAt: Date
  isPrimary: boolean
}

const COMMAND_QUEUE_SIZE = 10
const HEARTBEAT_INTERVAL_MS = 30_000
const PONG_TIMEOUT_MS = 5_000

export class ViewerBridge {
  private wss: WebSocketServer | null = null
  private clients: Map<string, ViewerClient> = new Map()
  private commandQueue: Array<{ timestamp: string; command: ViewerCommand }> = []
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null
  private port: number

  constructor(port: number) {
    this.port = port
  }

  async start(): Promise<void> {
    this.wss = new WebSocketServer({ port: this.port })

    this.wss.on("connection", (ws) => {
      const clientId = `viewer_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`

      // New client becomes primary if no others connected
      const isPrimary = this.clients.size === 0
      // Demote all existing clients from primary if new one connects
      if (isPrimary) {
        for (const c of this.clients.values()) c.isPrimary = false
      }

      const client: ViewerClient = {
        id: clientId,
        ws,
        connectedAt: new Date(),
        lastPongAt: new Date(),
        isPrimary,
      }
      this.clients.set(clientId, client)
      console.error(`[ViewerBridge] ${clientId} connected (${this.clients.size} total, primary=${isPrimary})`)

      // Send queued commands to new connection
      for (const { command } of this.commandQueue) {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify(command))
        }
      }

      ws.on("message", (data) => {
        try {
          const msg = JSON.parse(data.toString())
          if (msg.type === "pong") {
            client.lastPongAt = new Date()
          }
        } catch {
          /* ignore malformed */
        }
      })

      ws.on("close", () => {
        this.clients.delete(clientId)
        console.error(`[ViewerBridge] ${clientId} disconnected (${this.clients.size} remaining)`)
        // Promote another client to primary
        const remaining = Array.from(this.clients.values())
        if (remaining.length > 0 && !remaining.some((c) => c.isPrimary)) {
          remaining[remaining.length - 1].isPrimary = true
        }
      })

      ws.on("error", (err) => {
        console.error(`[ViewerBridge] ${clientId} error:`, err.message)
        this.clients.delete(clientId)
      })
    })

    // Heartbeat loop
    this.heartbeatTimer = setInterval(() => {
      const now = Date.now()
      for (const [id, client] of this.clients) {
        if (client.ws.readyState !== WebSocket.OPEN) {
          this.clients.delete(id)
          continue
        }
        const timeSincePong = now - client.lastPongAt.getTime()
        if (timeSincePong > HEARTBEAT_INTERVAL_MS + PONG_TIMEOUT_MS) {
          console.error(`[ViewerBridge] ${id} pong timeout — terminating`)
          client.ws.terminate()
          this.clients.delete(id)
          continue
        }
        client.ws.send(JSON.stringify({ type: "ping" }))
      }
    }, HEARTBEAT_INTERVAL_MS)

    console.error(`[ViewerBridge] Listening on ws://localhost:${this.port}`)
  }

  sendCommand(command: ViewerCommand): void {
    // Add to queue (drop oldest if at capacity)
    this.commandQueue.push({ timestamp: new Date().toISOString(), command })
    if (this.commandQueue.length > COMMAND_QUEUE_SIZE) {
      this.commandQueue.shift()
    }

    const msg = JSON.stringify(command)
    let sent = 0
    for (const client of this.clients.values()) {
      if (client.ws.readyState === WebSocket.OPEN) {
        client.ws.send(msg)
        sent++
      }
    }
    console.error(`[ViewerBridge] Sent ${command.type} to ${sent}/${this.clients.size} clients`)
  }

  getCommandHistory() {
    return this.commandQueue
  }
  get connectedClients(): number {
    return this.clients.size
  }
  get primaryClientId(): string | undefined {
    return Array.from(this.clients.values()).find((c) => c.isPrimary)?.id
  }

  async stop(): Promise<void> {
    if (this.heartbeatTimer) clearInterval(this.heartbeatTimer)
    this.wss?.close()
  }
}
```

**Update the viewer's `ws_client.ts`** to respond to heartbeat pings:

In `apps/tilegraph-viewer/src/agent/ws_client.ts`, inside the `ws.onmessage` handler, add before the `switch` statement:

```typescript
if (cmd.type === "ping") {
  this.ws!.send(JSON.stringify({ type: "pong" }))
  return
}
```

---

## Stage 4.4 — Audit log persistence and session queries

### What to change in `src/audit/logger.ts`

```typescript
import { appendFile, mkdir, stat, rename } from "fs/promises"
import { existsSync, readFileSync } from "fs"
import { dirname } from "path"

const MAX_LOG_SIZE_BYTES = 10 * 1024 * 1024 // 10MB

export interface AuditEntry {
  session_id: string
  timestamp: string
  tool_name: string
  input: unknown
  output_summary: string
  duration_ms: number
  error?: string
}

export class AuditLogger {
  private path: string
  private sessionId: string
  private callCount = 0
  private totalDurationMs = 0

  constructor(path: string) {
    this.path = path
    this.sessionId = `session_${Date.now()}`
  }

  async log(entry: Omit<AuditEntry, "session_id" | "timestamp">): Promise<void> {
    this.callCount++
    this.totalDurationMs += entry.duration_ms

    const full: AuditEntry = {
      ...entry,
      session_id: this.sessionId,
      timestamp: new Date().toISOString(),
    }
    try {
      await mkdir(dirname(this.path), { recursive: true })
      await this.rotateIfNeeded()
      await appendFile(this.path, JSON.stringify(full) + "\n", "utf-8")
    } catch (err) {
      console.error("[AuditLogger] Failed to write:", err)
    }
  }

  private async rotateIfNeeded(): Promise<void> {
    if (!existsSync(this.path)) return
    try {
      const { size } = await stat(this.path)
      if (size > MAX_LOG_SIZE_BYTES) {
        const rotatedPath = this.path.replace(/(\.\w+)?$/, `.${Date.now()}$1`)
        await rename(this.path, rotatedPath)
        console.error(`[AuditLogger] Rotated to ${rotatedPath}`)
      }
    } catch {
      /* ignore */
    }
  }

  /** Read all entries for a specific session. */
  getSessionEntries(sessionId: string): AuditEntry[] {
    if (!existsSync(this.path)) return []
    try {
      return readFileSync(this.path, "utf-8")
        .split("\n")
        .filter(Boolean)
        .map((line) => JSON.parse(line) as AuditEntry)
        .filter((e) => e.session_id === sessionId)
    } catch {
      return []
    }
  }

  /** Read the last N entries across all sessions. */
  getLastEntries(n: number): AuditEntry[] {
    if (!existsSync(this.path)) return []
    try {
      const all = readFileSync(this.path, "utf-8")
        .split("\n")
        .filter(Boolean)
        .map((line) => JSON.parse(line) as AuditEntry)
      return all.slice(-n)
    } catch {
      return []
    }
  }

  getSessionId(): string {
    return this.sessionId
  }
  getSessionSummary() {
    return {
      session_id: this.sessionId,
      tool_call_count: this.callCount,
      total_duration_ms: this.totalDurationMs,
    }
  }
}
```

**Update `src/resources/index.ts`** to expose audit resources:

```typescript
server.setRequestHandler(ListResourcesRequestSchema, async () => ({
  resources: [
    { uri: "tilegraph://model/summary", name: "Plant model summary", mimeType: "application/json" },
    { uri: "tilegraph://selection/current", name: "Current viewer selection", mimeType: "application/json" },
    {
      uri: `tilegraph://audit/session/${ctx.auditLogger.getSessionId()}`,
      name: "Current session audit log",
      mimeType: "application/json",
    },
    { uri: "tilegraph://audit/last/20", name: "Last 20 audit entries", mimeType: "application/json" },
  ],
}))

// In the ReadResource handler, add these cases:
if (uri.startsWith("tilegraph://audit/session/")) {
  const sessionId = uri.replace("tilegraph://audit/session/", "")
  const entries = ctx.auditLogger.getSessionEntries(sessionId)
  return {
    contents: [
      {
        uri,
        mimeType: "application/json",
        text: JSON.stringify(
          {
            session_id: sessionId,
            entry_count: entries.length,
            summary: ctx.auditLogger.getSessionSummary(),
            entries,
          },
          null,
          2,
        ),
      },
    ],
  }
}

if (uri.startsWith("tilegraph://audit/last/")) {
  const n = parseInt(uri.replace("tilegraph://audit/last/", "")) || 20
  const entries = ctx.auditLogger.getLastEntries(n)
  return {
    contents: [
      {
        uri,
        mimeType: "application/json",
        text: JSON.stringify({ entries }, null, 2),
      },
    ],
  }
}
```

---

## Verification sequence

```bash
cd apps/tilegraph-mcp-server

# 1. TypeScript compile check
npm run build
# Must compile with 0 errors

# 2. Run tests
npm run test
# All vitest tests must pass

# 3. Start server and verify startup
npm run dev &
sleep 3

# 4. Check health endpoint (if REST API from Prompt 3 is also implemented)
curl http://localhost:9000/health | python3 -m json.tool
# Expected: {"status":"ok","neo4j":{"connected":true,"latency_ms":...}}

# 5. Verify Zod validation rejects bad input
# Run MCP client test or curl the test endpoint:
node -e "
const { z } = require('zod');
// Simulate the TagSchema
const TagSchema = z.string().min(1).max(64).regex(/^[A-Z0-9\\-_\\.]+$/i);
try {
    TagSchema.parse('P-1001'); console.log('valid tag: OK');
    TagSchema.parse('LINE 1001'); // has space — should throw
} catch (e) { console.log('invalid tag rejected: OK'); }
"

# 6. Test WebSocket heartbeat manually
node -e "
const WebSocket = require('ws');
const ws = new WebSocket('ws://localhost:9001');
ws.on('message', (data) => {
    const msg = JSON.parse(data);
    if (msg.type === 'ping') {
        console.log('Received ping, sending pong');
        ws.send(JSON.stringify({ type: 'pong' }));
    }
});
ws.on('open', () => console.log('Connected'));
setTimeout(() => { ws.close(); process.exit(0); }, 35000);
"
# Should print: "Received ping, sending pong" after ~30 seconds

# 7. Verify audit log entries
ls -la output/reports/audit.jsonl 2>/dev/null || echo "No audit log yet — run some tool calls first"
```

**Done when:**

- `npm run build` compiles with 0 errors
- `npm run test` passes
- `Neo4jClient.healthCheck()` returns `{ connected: true }` when Neo4j is running
- Zod validation rejects malformed `object_id` (e.g., missing `obj_` prefix) and returns `error_code: "VALIDATION_ERROR"`
- WebSocket clients receive `{ type: "ping" }` every 30s
- New viewer connections receive the last 10 queued commands on connect
- `tilegraph://audit/session/{id}` MCP resource returns entries with `tool_call_count` in summary
