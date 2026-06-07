# Migration Plan — Remove neo4j-driver, complete HTTP-only Neo4j path

## Inventory Summary

| File                   | Runtime           | Framework                  | Neo4j client               | Status                      |
| ---------------------- | ----------------- | -------------------------- | -------------------------- | --------------------------- |
| `src/worker.ts`        | Cloudflare Worker | Hono ✓                     | `Neo4jHttpClient` (HTTP) ✓ | **Complete — do not touch** |
| `src/index.ts`         | Node.js local dev | Hono + @hono/node-server ✓ | `Neo4jClient` (Bolt) ✗     | **Target**                  |
| `src/db/neo4j_http.ts` | Both              | —                          | HTTP fetch() client        | Keep                        |
| `src/db/neo4j.ts`      | Node.js only      | —                          | neo4j-driver (Bolt)        | **Delete**                  |

Express was already removed from `src/index.ts` (plan in `hono_plan.md` is complete).  
`hono` and `@hono/node-server` are already in `package.json`.  
The only remaining migration: Bolt driver → HTTP in `src/index.ts`.

---

## Stage 1 — Update src/index.ts

**File:** `src/index.ts`

Replace:

- `import { Neo4jClient } from "./db/neo4j.js"` → `import { Neo4jHttpClient } from "./db/neo4j_http.js"`
- Constructor call: `new Neo4jClient({ url: process.env.NEO4J_URL, username: process.env.NEO4J_USER, ... })`
  → `new Neo4jHttpClient({ url: process.env.NEO4J_URL ?? "http://localhost:7474", username: process.env.NEO4J_USERNAME ?? "neo4j", ... })`
- Fix env var name: `NEO4J_USER` → `NEO4J_USERNAME` (matches `.dev.vars`)
- Fix default URL: `"bolt://localhost:7687"` → `"http://localhost:7474"`

The `Neo4jHttpClient` exposes identical method signatures to `Neo4jClient` — no other changes in `index.ts`.

**Checklist:**

- [ ] Replace import
- [ ] Replace constructor call with `Neo4jHttpClient`
- [ ] Fix `NEO4J_USER` → `NEO4J_USERNAME` env var
- [ ] Fix default URL to `http://localhost:7474`

---

## Stage 2 — Delete src/db/neo4j.ts

After Stage 1, `src/db/neo4j.ts` (Bolt-based client) has zero importers.

**Checklist:**

- [ ] Delete `src/db/neo4j.ts`

---

## Stage 3 — Remove neo4j-driver from package.json

**Checklist:**

- [ ] Run `bun remove neo4j-driver`
- [ ] Verify `neo4j-driver` no longer appears in `package.json`

---

## Stage 4 — Build verification

**Checklist:**

- [ ] `bun run build` exits 0 (tsc)
- [ ] `grep -r "neo4j-driver" src/` returns empty
- [ ] `grep -r "Neo4jClient[^H]" src/` returns empty (only `Neo4jHttpClient` remains)

---

## Stage 5 — Test suite

**Checklist:**

- [ ] `bun run test` exits 0 (vitest)

---

## Completion Definition

Migration is complete when all of the following are true:

1. `grep "neo4j-driver" package.json` returns nothing
2. `grep -r "from.*neo4j\.js" src/` returns nothing
3. `bun run build` exits 0
4. `bun run test` exits 0
5. `src/worker.ts` is **untouched**
