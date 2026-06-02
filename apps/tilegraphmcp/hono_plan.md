# Hono Migration Plan — Remove Express from tilegraphmcp

## Context

`apps/tilegraphmcp` has **two entry points**:

| File            | Runtime                         | Current framework                      |
| --------------- | ------------------------------- | -------------------------------------- |
| `src/worker.ts` | Cloudflare Worker               | **Hono already** — nothing to do       |
| `src/index.ts`  | Node.js (local dev + stdio MCP) | **Express** — target of this migration |

The Cloudflare Worker path (`worker.ts`) already uses Hono correctly and is the production reference.  
The local dev path (`index.ts`) still imports `express` for the companion REST API that runs alongside the stdio MCP server.

**Goal:** Replace the Express REST API in `src/index.ts` with Hono + `@hono/node-server` so Express is completely removed from the project.

---

## Express Surface in src/index.ts (lines to change)

| Line(s)                                                            | Express construct           | Hono equivalent                                                                                          |
| ------------------------------------------------------------------ | --------------------------- | -------------------------------------------------------------------------------------------------------- |
| `import express from "express"`                                    | Express import              | `import { Hono } from "hono"` + `import { serve } from "@hono/node-server"`                              |
| `const app = express()`                                            | App instance                | `const app = new Hono()`                                                                                 |
| `app.use(express.json())`                                          | JSON body parser middleware | Remove — Hono parses via `c.req.json()` on demand                                                        |
| `res.header("Access-Control-Allow-Origin", "*")` inline middleware | CORS                        | `app.use('*', cors())` from `hono/cors`                                                                  |
| `(req, res) => { res.status(N).json({}) }` handler signature       | Express handler             | `(c) => { return c.json({}, N) }`                                                                        |
| `req.params.id`                                                    | Route params                | `c.req.param("id")`                                                                                      |
| `req.body as { message?: string }`                                 | Parsed body                 | `await c.req.json()`                                                                                     |
| `res.setHeader(...)` + `res.write(...)` + `res.end()` (SSE)        | Node.js stream              | `return new Response(readable, { headers })` via `TransformStream` — same pattern already in `worker.ts` |
| `app.listen(REST_PORT, callback)`                                  | Start server                | `serve({ fetch: app.fetch, port: REST_PORT })`                                                           |

**Zero changes needed** outside `src/index.ts` and `package.json`.

---

## Packages

### Add

- `@hono/node-server` — Hono adapter for Node.js (wraps `node:http`; same Hono app runs on both Workers and Node)

### Remove

- `express` (dependency)
- `@types/express` (devDependency)

---

## Staged Plan

---

### Stage 1 — Dependency swap

**Files:** `package.json`

**Checklist:**

- [ ] Add `@hono/node-server` to `dependencies`
- [ ] Remove `express` from `dependencies`
- [ ] Remove `@types/express` from `devDependencies`
- [ ] Run `bun install` to regenerate lockfile
- [ ] Confirm `bun run build` (`tsc`) still passes with no Express type references

**Constraint:** `hono` is already present at `^4.12.23`. `@hono/node-server` must be on the same major (`^4.x`). Check the latest compatible version before installing.

---

### Stage 2 — Migrate src/index.ts REST API

**File:** `src/index.ts`

This is a line-by-line swap of the Express REST server with the Hono equivalent. Use `src/worker.ts` as the reference — the route logic is identical, only the handler signature and streaming pattern change.

#### 2.1 Imports

```diff
-import express from "express";
+import { Hono } from "hono";
+import { cors } from "hono/cors";
+import { serve } from "@hono/node-server";
```

#### 2.2 App instantiation + CORS

Replace:

```typescript
const app = express()
app.use(express.json())
app.use((_req, res, next) => {
  res.header("Access-Control-Allow-Origin", "*")
  res.header("Access-Control-Allow-Headers", "Content-Type")
  next()
})
```

With:

```typescript
const app = new Hono()
app.use("*", cors())
```

#### 2.3 GET /objects/:id

Replace Express handler signature `(req, res) =>` with `(c) =>`:

```typescript
app.get("/objects/:id", async (c) => {
  try {
    const results = await neo4j.getObjectProperties(c.req.param("id"))
    if (results.length === 0) {
      return c.json({ error: "NOT_FOUND", object_id: c.req.param("id") }, 404)
    }
    const row = results[0] as Record<string, unknown>
    const obj = (row.o ?? row) as Record<string, unknown>
    const props = (obj as any).properties ?? obj
    return c.json({ found: true, object_id: c.req.param("id"), properties: props })
  } catch (err) {
    return c.json({ error: "GRAPH_UNAVAILABLE", message: String(err) }, 503)
  }
})
```

#### 2.4 GET /health

```typescript
app.get("/health", async (c) => {
  const check = await neo4j.healthCheck()
  return c.json({
    status: check.connected ? "ok" : "degraded",
    neo4j: check,
    spatial_index_records: spatialIndex.count,
  })
})
```

#### 2.5 GET /hierarchy

Replace `res.status(503).json(...)` with `return c.json(..., 503)` and `res.json(...)` with `return c.json(...)`. The query + tree-building logic is unchanged.

#### 2.6 POST /chat (SSE)

This is the most important change. Replace the Express streaming pattern with the Web Streams pattern already used in `worker.ts`:

Replace:

```typescript
app.post("/chat", async (req, res) => {
  // ...
  res.setHeader("Content-Type", "text/event-stream")
  res.setHeader("Cache-Control", "no-cache")
  res.setHeader("Connection", "keep-alive")
  res.setHeader("Access-Control-Allow-Origin", "*")
  const sendChunk = (data: string) => {
    res.write(`data: ...`)
  }
  // ...
  res.end()
})
```

With (identical pattern to `worker.ts` lines 162–199):

```typescript
app.post("/chat", async (c) => {
  const body = (await c.req.json().catch(() => ({}))) as { message?: string }
  const message = body?.message
  if (!message || typeof message !== "string" || !message.trim()) {
    return c.json({ error: "VALIDATION_ERROR", message: "message is required" }, 400)
  }

  const { readable, writable } = new TransformStream()
  const writer = writable.getWriter()
  const encoder = new TextEncoder()
  const sseEvent = (payload: unknown) => writer.write(encoder.encode(`data: ${JSON.stringify(payload)}\n\n`))

  ;(async () => {
    try {
      const turns = await runAgentLoop(message.trim(), { neo4j, spatialIndex, viewerBridge, auditLogger }, (chunk) =>
        sseEvent({ type: "chunk", text: chunk }),
      )
      const toolCallNames = turns.flatMap((t) => t.tool_calls ?? []).map((tc) => tc.name)
      await sseEvent({
        type: "done",
        turns: turns.length,
        tool_calls: toolCallNames,
        session_id: auditLogger.getSessionId(),
      })
    } catch (err: any) {
      await sseEvent({ type: "error", message: err.message ?? String(err) })
    } finally {
      await writer.close()
    }
  })()

  return new Response(readable, {
    headers: {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      "X-Accel-Buffering": "no",
    },
  })
})
```

**Note:** `@hono/node-server` v4 supports Web Standard `Response` with `ReadableStream` on Node.js — no polyfill needed.

#### 2.7 Server start

Replace:

```typescript
app.listen(REST_PORT, () => {
  console.error(`[REST API] listening on http://localhost:${REST_PORT}`)
})
```

With:

```typescript
serve({ fetch: app.fetch, port: REST_PORT }, () => {
  console.error(`[REST API] listening on http://localhost:${REST_PORT}`)
})
```

**Checklist:**

- [ ] Remove `express` import
- [ ] Add `Hono`, `cors`, `serve` imports
- [ ] Replace `express()` + `express.json()` + inline CORS middleware
- [ ] Migrate `GET /objects/:id` handler
- [ ] Migrate `GET /health` handler
- [ ] Migrate `GET /hierarchy` handler
- [ ] Migrate `POST /chat` SSE handler — use TransformStream pattern from worker.ts
- [ ] Replace `app.listen(...)` with `serve({ fetch: app.fetch, port: REST_PORT })`
- [ ] Confirm no residual `req`, `res` Express-style parameters remain

---

### Stage 3 — TypeScript build verification

**Checklist:**

- [ ] `bun run build` passes with zero TypeScript errors
- [ ] No `express` or `@types/express` symbols appear in type output
- [ ] `grep -r "from 'express'\|from \"express\"" src/` returns empty

---

### Stage 4 — Runtime smoke test (local dev)

**Checklist:**

- [ ] `bun run dev` starts without errors
- [ ] `curl http://localhost:9000/health` returns `{ "status": "ok"|"degraded", ... }` JSON
- [ ] `curl http://localhost:9000/objects/test-id` returns `{ "error": "NOT_FOUND" }` with HTTP 404
- [ ] `curl -N -X POST http://localhost:9000/chat -H 'Content-Type: application/json' -d '{"message":"hello"}' ` produces SSE `data:` lines
- [ ] Missing `message` body returns HTTP 400 with `VALIDATION_ERROR`
- [ ] MCP stdio transport still starts and accepts tool calls (index.ts non-HTTP path unchanged)

---

### Stage 5 — Test suite

**Checklist:**

- [ ] `bun run test` passes (vitest — existing tests are unit tests on audit logger and validation schemas; they do not import express or the HTTP server, so no changes expected)
- [ ] If any test imports `express` or mocks `express`, update those imports

---

## Completion Definition

Migration is **complete** when all of the following are true:

1. `grep -r "express" apps/tilegraphmcp/package.json` returns nothing
2. `grep -r "from 'express'\|from \"express\"\|require.*express" apps/tilegraphmcp/src/` returns nothing
3. `bun run build` exits 0
4. `bun run test` exits 0
5. Local dev server starts and all 5 curl smoke tests pass (Stage 4 checklist)
6. `src/worker.ts` is **untouched** — Cloudflare Worker path must not change

---

## Risk Notes

- **TransformStream on Node.js**: Available natively since Node 18. The project already targets ES modules and modern Node, so no polyfill is needed. `@hono/node-server` handles this correctly.
- **SSE `Connection: keep-alive`**: Express sets this explicitly. `@hono/node-server` keeps connections alive by default for streaming responses; the `X-Accel-Buffering: no` header is sufficient for proxies.
- **`serve()` return type**: `@hono/node-server`'s `serve()` returns a `ServerType` (Node `http.Server`). If `main()` needs to close the server on shutdown, store the reference.
- **CORS preflight on `/chat`**: The `cors()` middleware in Hono handles `OPTIONS` preflight automatically. The existing Express approach set headers inline per route, which skipped preflight responses. Hono's `cors()` is strictly more correct.
- **No behavior change for non-HTTP code**: `Server`, `StdioServerTransport`, `registerTools`, `registerResources`, `ViewerBridge`, `AuditLogger`, and `Neo4jClient` are all unchanged — only the HTTP server bootstrap differs.
