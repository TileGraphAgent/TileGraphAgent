# Prompt 5 — Agent Chat: Wire to Real Claude API

## Your role

You are implementing production improvements to **TileGraphAgent**. This session covers **Project 5, Stage 5.5** and **Project 4, Stage 4.5** from `plan.md`:

1. Wire the viewer's "Ask" button to a real Claude API session via streaming SSE
2. Add a `POST /chat` endpoint to the MCP server that executes the agent loop
3. Write an end-to-end integration test that verifies the agent calls tools in the correct order

**Prerequisites:**

- Prompts 1–4 complete (pipeline compiles, MCP server hardens, viewer picks objects)
- Neo4j running with graph data imported
- `ANTHROPIC_API_KEY` environment variable set

## Repository overview

- **MCP server:** `apps/tilegraph-mcp-server/`
- **Viewer:** `apps/tilegraph-viewer/`
- **Agent system prompt:** `docs/mcp/agent_system_prompt.md`

**Install the Anthropic SDK:**

```bash
cd apps/tilegraph-mcp-server
npm install @anthropic-ai/sdk
```

---

## Stage 5.5 — Backend: `POST /chat` endpoint with Claude + tool execution

### Step 1 — Create `src/agent/claude_agent.ts`

This module runs the full agent loop: receives a user message, calls Claude with tool definitions, executes tool calls through the existing MCP tool handlers, and streams the final response.

```typescript
import Anthropic from "@anthropic-ai/sdk"
import { readFileSync } from "fs"
import { join } from "path"
import type { ToolContext } from "../tools/index.js"
import { TOOLS } from "../tools/index.js"

const SYSTEM_PROMPT_PATH = join(new URL(".", import.meta.url).pathname, "../../../docs/mcp/agent_system_prompt.md")

function loadSystemPrompt(): string {
  try {
    return readFileSync(SYSTEM_PROMPT_PATH, "utf-8")
  } catch {
    return "You are TileGraphAgent. Always use tools to retrieve engineering data. Never infer facts."
  }
}

// Convert MCP tool definitions to Anthropic tool format
function mcpToolsToAnthropicTools(): Anthropic.Tool[] {
  return TOOLS.map((t) => ({
    name: t.definition.name,
    description: t.definition.description,
    input_schema: t.definition.inputSchema as Anthropic.Tool["input_schema"],
  }))
}

export interface AgentTurn {
  role: "user" | "assistant"
  content: string
  tool_calls?: { name: string; input: unknown; result: unknown }[]
}

export async function runAgentLoop(
  userMessage: string,
  ctx: ToolContext,
  onChunk: (chunk: string) => void,
  maxToolRounds = 8,
): Promise<AgentTurn[]> {
  const client = new Anthropic()
  const systemPrompt = loadSystemPrompt()
  const anthropicTools = mcpToolsToAnthropicTools()

  const turns: AgentTurn[] = []
  const messages: Anthropic.MessageParam[] = [{ role: "user", content: userMessage }]

  for (let round = 0; round < maxToolRounds; round++) {
    const response = await client.messages.create({
      model: "claude-sonnet-4-6",
      max_tokens: 4096,
      system: systemPrompt,
      tools: anthropicTools,
      messages,
    })

    // Stream text chunks
    let assistantText = ""
    const toolUses: Anthropic.ToolUseBlock[] = []

    for (const block of response.content) {
      if (block.type === "text") {
        assistantText += block.text
        // Emit text progressively (chunk by sentence for better UX)
        onChunk(block.text)
      } else if (block.type === "tool_use") {
        toolUses.push(block)
      }
    }

    // Record assistant turn
    turns.push({
      role: "assistant",
      content: assistantText,
      tool_calls: [],
    })

    // If no tool calls, we're done
    if (toolUses.length === 0 || response.stop_reason === "end_turn") {
      break
    }

    // Execute each tool call
    const toolResults: Anthropic.ToolResultBlockParam[] = []

    for (const toolUse of toolUses) {
      const tool = TOOLS.find((t) => t.definition.name === toolUse.name)
      let result: unknown

      if (!tool) {
        result = { error_code: "UNKNOWN_TOOL", message: `Tool '${toolUse.name}' not found` }
      } else {
        const t0 = Date.now()
        try {
          result = await tool.handler(toolUse.input as Record<string, unknown>, ctx)
          await ctx.auditLogger.log({
            tool_name: toolUse.name,
            input: toolUse.input,
            output_summary: JSON.stringify(result).slice(0, 200),
            duration_ms: Date.now() - t0,
          })
        } catch (err) {
          result = {
            error_code: "TOOL_ERROR",
            message: err instanceof Error ? err.message : String(err),
          }
          await ctx.auditLogger.log({
            tool_name: toolUse.name,
            input: toolUse.input,
            output_summary: "TOOL_ERROR",
            duration_ms: Date.now() - t0,
            error: result.error_code as string,
          })
        }
      }

      turns[turns.length - 1].tool_calls!.push({
        name: toolUse.name,
        input: toolUse.input,
        result,
      })

      toolResults.push({
        type: "tool_result",
        tool_use_id: toolUse.id,
        content: JSON.stringify(result),
      })

      // Stream tool execution feedback to client
      onChunk(`\n[Tool: ${toolUse.name}]\n`)
    }

    // Add assistant + tool results to message history
    messages.push({ role: "assistant", content: response.content })
    messages.push({ role: "user", content: toolResults })
  }

  return turns
}
```

**Export `TOOLS` array from `src/tools/index.ts`** (it's currently local — add `export const TOOLS = [...]`).

### Step 2 — Add `POST /chat` to `src/index.ts`

In the Express REST server section:

```typescript
app.post("/chat", async (req, res) => {
  const { message, session_id } = req.body as { message?: string; session_id?: string }

  if (!message || typeof message !== "string" || message.trim().length === 0) {
    return res.status(400).json({ error: "VALIDATION_ERROR", message: "message is required" })
  }

  // Set headers for SSE streaming
  res.setHeader("Content-Type", "text/event-stream")
  res.setHeader("Cache-Control", "no-cache")
  res.setHeader("Connection", "keep-alive")
  res.setHeader("Access-Control-Allow-Origin", "*")

  const sendChunk = (data: string) => {
    res.write(`data: ${JSON.stringify({ type: "chunk", text: data })}\n\n`)
  }

  try {
    const turns = await runAgentLoop(message.trim(), { neo4j, spatialIndex, viewerBridge, auditLogger }, sendChunk)

    // Send final summary event
    const toolCallNames = turns.flatMap((t) => t.tool_calls ?? []).map((tc) => tc.name)

    res.write(
      `data: ${JSON.stringify({
        type: "done",
        turns: turns.length,
        tool_calls: toolCallNames,
        session_id: auditLogger.getSessionId(),
      })}\n\n`,
    )
  } catch (err: any) {
    res.write(
      `data: ${JSON.stringify({
        type: "error",
        message: err.message ?? String(err),
      })}\n\n`,
    )
  } finally {
    res.end()
  }
})
```

---

## Stage 5.5 (viewer) — Wire "Ask" button to SSE stream

### What to add in `apps/tilegraph-viewer/src/agent/claude_client.ts`

```typescript
const AGENT_API_BASE = import.meta.env.VITE_MCP_REST_URL ?? "http://localhost:9000"

export interface AgentChunk {
  type: "chunk" | "done" | "error"
  text?: string
  turns?: number
  tool_calls?: string[]
  session_id?: string
  message?: string
}

export async function sendAgentMessage(
  message: string,
  onChunk: (chunk: AgentChunk) => void,
  signal?: AbortSignal,
): Promise<void> {
  const res = await fetch(`${AGENT_API_BASE}/chat`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ message }),
    signal,
  })

  if (!res.ok) {
    const err = await res.json().catch(() => ({ message: res.statusText }))
    onChunk({ type: "error", message: err.message ?? "Unknown error" })
    return
  }

  const reader = res.body!.getReader()
  const decoder = new TextDecoder()
  let buffer = ""

  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true })

    // Parse SSE lines
    const lines = buffer.split("\n")
    buffer = lines.pop() ?? ""

    for (const line of lines) {
      if (line.startsWith("data: ")) {
        try {
          const chunk = JSON.parse(line.slice(6)) as AgentChunk
          onChunk(chunk)
        } catch {
          /* skip malformed */
        }
      }
    }
  }
}
```

### Update `apps/tilegraph-viewer/src/main.ts`

Wire the submit button:

```typescript
import { sendAgentMessage } from "./agent/claude_client.js"

// After all other initializations, add:
const agentInput = document.getElementById("agent-input") as HTMLInputElement
const agentSubmit = document.getElementById("agent-submit") as HTMLButtonElement
const agentMessages = document.getElementById("agent-messages")!

let agentAbortController: AbortController | null = null

function appendAgentMessage(role: "user" | "assistant", text: string): HTMLElement {
  const div = document.createElement("div")
  div.className = `msg-${role}`
  div.textContent = text
  agentMessages.appendChild(div)
  agentMessages.scrollTop = agentMessages.scrollHeight
  return div
}

agentSubmit.addEventListener("click", async () => {
  const message = agentInput.value.trim()
  if (!message) return

  agentInput.value = ""
  agentSubmit.disabled = true
  store.update({ isAgentProcessing: true })

  appendAgentMessage("user", message)
  const assistantDiv = appendAgentMessage("assistant", "")
  let assistantText = ""

  agentAbortController = new AbortController()

  try {
    await sendAgentMessage(
      message,
      (chunk) => {
        if (chunk.type === "chunk" && chunk.text) {
          assistantText += chunk.text
          assistantDiv.textContent = assistantText
          agentMessages.scrollTop = agentMessages.scrollHeight
        } else if (chunk.type === "done") {
          const toolSummary = chunk.tool_calls?.join(", ") ?? "none"
          const meta = document.createElement("div")
          meta.className = "msg-meta"
          meta.textContent = `[${chunk.turns} turns, tools: ${toolSummary}]`
          agentMessages.appendChild(meta)
        } else if (chunk.type === "error") {
          assistantDiv.textContent = `Error: ${chunk.message}`
          assistantDiv.style.color = "#ef5350"
        }
      },
      agentAbortController.signal,
    )
  } finally {
    agentSubmit.disabled = false
    store.update({ isAgentProcessing: false })
    agentAbortController = null
  }
})

// Allow Enter key to submit
agentInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault()
    agentSubmit.click()
  }
})
```

Add CSS in `index.html` for the meta line:

```css
.msg-meta {
  font-size: 10px;
  color: #37474f;
  margin: 2px 0 6px 0;
  font-style: italic;
}
```

---

## Stage 4.5 — End-to-end integration test

**Create `apps/tilegraph-mcp-server/tests/integration/`** directory.

**Create `apps/tilegraph-mcp-server/tests/integration/mock_neo4j.ts`:**

```typescript
// In-memory Neo4j mock that returns fixed data for test tags
export class MockNeo4jClient {
  async query(cypher: string, params: Record<string, unknown> = {}) {
    // Handle tag lookup for LINE-1001
    if (cypher.includes("tag: $tag") && params.tag === "LINE-1001") {
      return [
        {
          o: {
            properties: {
              object_id: "obj_test_line_1001",
              tag: "LINE-1001",
              name: "LINE-1001",
              class: "Line",
              status: "Active",
              tile_id: "area-a/content",
              feature_id: 42,
            },
          },
        },
      ]
    }
    if (cypher.includes("CONNECTED_TO") && cypher.includes("LINE-1001")) {
      return [
        {
          object_id: "obj_test_pump_1001",
          tag: "P-10101",
          class: "Pump",
          rel_type: "CONNECTED_TO",
        },
      ]
    }
    if (cypher.includes("maintenance") || cypher.includes("ISOLATED_BY")) {
      return [
        {
          line_tag: "LINE-1001",
          line_id: "obj_test_line_1001",
          connected_pumps: ["P-10101"],
          isolation_valves: ["V-10101A", "V-10101B"],
          instruments: ["FT-10101"],
          segment_count: 16,
        },
      ]
    }
    return []
  }
  async healthCheck() {
    return { connected: true, latency_ms: 1 }
  }
  async close() {}

  // Add stubs for all query methods used by tools
  async findObjectByTag(tag: string) {
    return this.query("tag: $tag", { tag })
  }
  async getObjectProperties(id: string) {
    return [{ o: { properties: { object_id: id, tag: "TEST", class: "Pump" } } }]
  }
  async queryConnectedComponents(id: string) {
    return this.query("CONNECTED_TO", { id })
  }
  async queryUpstream(_id: string, _hops: number) {
    return []
  }
  async queryDownstream(_id: string, _hops: number) {
    return []
  }
  async pumpsConnectedToLine(lineTag: string) {
    return this.query("CONNECTED_TO LINE-1001", { lineTag })
  }
  async isolationValvesForLine(_lineTag: string) {
    return [
      { object_id: "obj_v_a", tag: "V-10101A", status: "Active", tile_id: "area-a/content", feature_id: 44 },
      { object_id: "obj_v_b", tag: "V-10101B", status: "Active", tile_id: "area-a/content", feature_id: 45 },
    ]
  }
  async maintenanceContextForLine(lineTag: string) {
    return this.query("maintenance", { lineTag })
  }
  async objectsInArea(_areaTag: string) {
    return []
  }
}
```

**Create `apps/tilegraph-mcp-server/tests/integration/tool_chain.test.ts`:**

```typescript
import { describe, it, expect, vi, beforeAll } from "vitest"
import { MockNeo4jClient } from "./mock_neo4j.js"
import { SpatialIndexClient } from "../../src/spatial/index.js"
import { ViewerBridge } from "../../src/viewer/bridge.js"
import { AuditLogger } from "../../src/audit/logger.js"
import type { ToolContext } from "../../src/tools/index.js"
import { TOOLS } from "../../src/tools/index.js"

// Skip if no API key — don't fail CI without credentials
const SKIP = !process.env.ANTHROPIC_API_KEY

function makeCtx(): ToolContext {
  const spatialIndex = new SpatialIndexClient("nonexistent.json")
  // Pre-populate with test data
  ;(spatialIndex as any).records = [
    {
      object_id: "obj_test_pump_1001",
      tag: "P-10101",
      class: "Pump",
      aabb_min: [1.0, 1.0, 0.0],
      aabb_max: [2.0, 2.0, 0.7],
      tile_id: "area-a/content",
      feature_id: 1201,
    },
  ]

  return {
    neo4j: new MockNeo4jClient() as any,
    spatialIndex,
    viewerBridge: {
      sendCommand: vi.fn(),
      connectedClients: 0,
      getCommandHistory: () => [],
    } as any,
    auditLogger: new AuditLogger("/tmp/test_audit.jsonl"),
  }
}

describe.skipIf(SKIP)("Agent tool chain integration", () => {
  it("resolves LINE-1001 and calls tools in correct order", async () => {
    const ctx = makeCtx()
    const { runAgentLoop } = await import("../../src/agent/claude_agent.js")

    const toolCallLog: string[] = []
    const originalHandlers = TOOLS.map((t) => ({ name: t.definition.name, handler: t.handler }))

    // Spy on each tool handler
    for (const tool of TOOLS) {
      const original = tool.handler
      tool.handler = async (args, c) => {
        toolCallLog.push(tool.definition.name)
        return original(args, c)
      }
    }

    const chunks: string[] = []
    await runAgentLoop(
      "Find all pumps connected to LINE-1001 and explain the maintenance impact.",
      ctx,
      (chunk) => chunks.push(chunk),
      6,
    )

    // Restore original handlers
    for (const { name, handler } of originalHandlers) {
      const tool = TOOLS.find((t) => t.definition.name === name)!
      tool.handler = handler
    }

    // Assert: search_object_by_tag must come before any graph query tool
    const searchIdx = toolCallLog.indexOf("search_object_by_tag")
    const graphIdx = toolCallLog.findIndex((n) =>
      ["query_connected_components", "query_upstream_downstream", "generate_maintenance_context"].includes(n),
    )
    expect(searchIdx, "search_object_by_tag must be called").toBeGreaterThanOrEqual(0)
    expect(searchIdx, "search must come before graph queries").toBeLessThan(graphIdx)

    // Assert: get_tile_feature_mapping before viewer tools
    const mappingIdx = toolCallLog.indexOf("get_tile_feature_mapping")
    const viewerIdx = toolCallLog.findIndex((n) =>
      ["highlight_objects_in_viewer", "isolate_system_in_viewer"].includes(n),
    )
    if (viewerIdx >= 0) {
      expect(mappingIdx, "feature mapping must precede viewer tools").toBeLessThan(viewerIdx)
    }

    // Assert: final answer mentions LINE-1001
    const fullText = chunks.join("")
    expect(fullText.toLowerCase()).toContain("line-1001")

    console.log("Tool call sequence:", toolCallLog.join(" → "))
    console.log("Response length:", fullText.length)
  }, 60_000) // 60s timeout for real API call
})
```

**Update `apps/tilegraph-mcp-server/package.json`** vitest config:

```json
"vitest": {
    "include": ["tests/**/*.test.ts"],
    "environment": "node",
    "testTimeout": 60000
}
```

---

## Verification sequence

```bash
cd apps/tilegraph-mcp-server

# 1. Install new dependency
npm install @anthropic-ai/sdk
npm run build
# Must compile with 0 errors

# 2. Run unit tests (no API key needed)
npm run test

# 3. Run integration test (requires API key)
ANTHROPIC_API_KEY=your_key npm run test -- tests/integration/tool_chain.test.ts
# Should print tool call sequence and PASS

# 4. Manual end-to-end demo
# Start Neo4j + import data
docker-compose up -d neo4j
# (import graph if not done)

# Start MCP server
npm run dev &
sleep 3

# Start viewer
cd ../tilegraph-viewer && npm run dev &

# Open http://localhost:5173
# Type in the agent panel: "Find all pumps connected to LINE-1001"
# Expected behavior:
# - Text streaming appears progressively
# - Tool calls are listed in brackets as they execute
# - Pumps and valves get highlighted in the 3D viewer
# - Final answer includes maintenance steps with evidence
```

**Done when:**

- `npm run build` compiles with 0 errors
- `npm run test` (unit tests) passes
- Typing a question in the viewer triggers tool calls visible in the audit panel
- The 3D viewer highlights objects as the agent runs
- `ANTHROPIC_API_KEY=... npm run test -- tests/integration/tool_chain.test.ts` passes
