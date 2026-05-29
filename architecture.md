# TileGraphAgent — System Architecture

## Overview

TileGraphAgent is an AI-driven industrial plant viewer that combines a **Rust data pipeline**, a **cloud-hosted knowledge graph**, and a **3D web viewer** into a single system. An LLM agent (Claude) navigates the plant by issuing tool calls that traverse the graph and drive the 3D visualization in real time.

### Deployment topology

```
┌─────────────────────── Local Machine ───────────────────────────┐
│                                                                 │
│  Rust Pipeline (cargo run --bin tilegraph)                      │
│   ├── generate-synth / IFC ingest                               │
│   ├── build-tiles  → output/tiles/  ──────────────┐             │
│   ├── build-graph  → output/graph/  ──────────────┼──▶ upload   │
│   └── validate                                    │             │
│                                                   ▼             │
│                                          wrangler r2 object put │
└───────────────────────────────────────────────────┼─────────────┘
                                                    │
                        ┌───────────────────────────▼──────────────────────────┐
                        │                  Cloudflare                          │
                        │                                                      │
                        │  R2 Bucket: tilegraph-data (public read)             │
                        │   ├── tiles/tileset.json                             │
                        │   ├── tiles/content/*.glb                            │
                        │   ├── tiles/index/spatial_index.json                 │
                        │   └── reports/audit.jsonl                            │
                        │                           │                          │
                        │  Worker: tilegraphmcp (Hono.js)              │
                        │   ├── MCP over HTTP-SSE  ◀────── Claude / AI agent   │
                        │   ├── REST /chat, /hierarchy, /objects/:id           │
                        │   ├── R2 binding → spatial_index.json                │
                        │   └── Durable Object: ViewerHub (WebSocket)          │
                        │                           │                          │
                        │  Pages: tilegraphviewer (CesiumJS + Vite)           │
                        │   ├── loads tiles from R2 (public URL)               │
                        │   ├── REST ──▶ Worker /chat, /hierarchy              │
                        │   └── WS   ──▶ Worker /ws/viewer (ViewerHub DO)      │
                        └───────────────────────────┼──────────────────────────┘
                                                    │
                        ┌───────────────────────────▼──────────────────────────┐
                        │  Neo4j Aura (managed cloud)                          │
                        │  neo4j+s://1c3578a5.databases.neo4j.io               │
                        │   Graph: EngObject nodes + relationships             │
                        └──────────────────────────────────────────────────────┘
```

---

## Components

### 1. Rust Pipeline (local)

The pipeline runs locally and populates all downstream data stores. It is not deployed to the cloud.

**Location:** repo root, `crates/`

**Stages (in order):**

| Stage                 | Command                       | Output                                  |
| --------------------- | ----------------------------- | --------------------------------------- |
| Synthetic generation  | `generate-synth`              | `output/synth/objects.json`             |
| IFC ingest (optional) | via `IfcAdapter`              | merged into normalized scene            |
| Geometry + GLB        | `build-tiles`                 | `output/tiles/content/*.glb`            |
| 3D Tiles manifest     | `build-tiles`                 | `output/tiles/tileset.json`             |
| Spatial index         | `build-tiles`                 | `output/tiles/index/spatial_index.json` |
| Graph export          | `build-graph`                 | `output/graph/import.cypher`            |
| Neo4j push            | `build-graph --push-to-neo4j` | ─▶ Neo4j Aura HTTP API                  |
| Validate              | `validate`                    | `output/reports/validation_report.json` |

**After each run, upload outputs to Cloudflare R2:**

```bash
# Upload tileset manifest
npx wrangler r2 object put tilegraph-data/tiles/tileset.json \
  --file output/tiles/tileset.json

# Upload spatial index (read by Worker at runtime)
npx wrangler r2 object put tilegraph-data/tiles/index/spatial_index.json \
  --file output/tiles/index/spatial_index.json

# Upload GLB content files
for f in output/tiles/content/*.glb; do
  npx wrangler r2 object put "tilegraph-data/tiles/content/$(basename $f)" --file "$f"
done

# Upload feature map
npx wrangler r2 object put tilegraph-data/tiles/metadata/tile_feature_map.json \
  --file output/tiles/metadata/tile_feature_map.json
```

**Neo4j push configuration** (`Neo4j-cert.txt`):

```env
NEO4J_URI=neo4j+s://1c3578a5.databases.neo4j.io
NEO4J_USERNAME=neo4j
NEO4J_PASSWORD=<password>
NEO4J_DATABASE=neo4j
AURA_INSTANCEID=1c3578a5
AURA_INSTANCENAME=My instance
```

The Rust `Neo4jClient` in `tilegraph-graph-export` uses Neo4j's **HTTP transactional endpoint** (`/db/neo4j/tx/commit`) over HTTPS, which is compatible with Aura without any additional driver.

---

### 2. Cloudflare R2 — Object Storage

**Bucket name:** `tilegraph-data`

R2 replaces the local filesystem as the canonical store for all pipeline outputs consumed at runtime. It provides S3-compatible APIs and public-read HTTPS URLs with no egress fees.

**Bucket layout:**

```
tilegraph-data/
  tiles/
    tileset.json                  ← 3D Tiles 1.1 root manifest
    content/
      {area}-piping.glb
      {area}-equipment.glb
      {area}-support.glb
      {area}-cable.glb
    metadata/
      tile_feature_map.json       ← feature_id → object_id mapping
    index/
      spatial_index.json          ← R-tree records for MCP spatial queries
  reports/
    audit.jsonl                   ← append-only MCP tool audit log
```

**Access patterns:**

| Consumer           | Access type              | What it reads                       |
| ------------------ | ------------------------ | ----------------------------------- |
| CesiumJS (browser) | Public HTTPS GET         | `tileset.json`, `*.glb`             |
| Cloudflare Worker  | R2 binding (internal)    | `spatial_index.json`, `audit.jsonl` |
| Rust pipeline      | `wrangler r2 object put` | writes all objects                  |

**CORS policy** (set on the bucket):

```json
[
  {
    "AllowedOrigins": ["https://tilegraphviewer.pages.dev", "http://localhost:5173"],
    "AllowedMethods": ["GET", "HEAD"],
    "AllowedHeaders": ["*"],
    "MaxAgeSeconds": 3600
  }
]
```

---

### 3. Cloudflare Workers — MCP Server (`tilegraphmcp`)

**Framework:** [Hono.js](https://hono.dev) — lightweight, TypeScript-native, Workers-first HTTP framework.

**Worker entry point:** `src/worker.ts`

The Worker serves two protocols:

#### 3a. MCP over HTTP-SSE

The `@modelcontextprotocol/sdk` `SSEServerTransport` exposes tools to Claude and other AI agents over standard HTTP:

```
GET  /sse       ← SSE stream (server → client events)
POST /messages  ← client → server tool calls
```

All 11 MCP tools are registered identically to the non-Worker version. Tool handlers receive a `ToolContext` with: `Neo4jClient`, `SpatialIndexClient`, `ViewerHubStub`, `AuditLogger`.

#### 3b. REST API (Hono routes)

```
GET  /health              → Neo4j latency + spatial index record count
GET  /objects/:id         → object properties from Neo4j
GET  /hierarchy           → area/system/line tree from Neo4j
POST /chat                → streaming SSE: Claude agent loop
GET  /ws/viewer           → WebSocket upgrade → Durable Object ViewerHub
```

These routes power the viewer's model tree, properties panel, and agent chat.

#### 3c. Neo4j Aura connectivity

The Workers runtime cannot open TCP connections, so the standard `neo4j-driver` Bolt protocol is not used. Instead, all Cypher is sent to Neo4j Aura's **HTTPS transactional API**:

```
POST https://1c3578a5.databases.neo4j.io:7473/db/neo4j/tx/commit
Authorization: Basic base64(neo4j:<password>)
Content-Type: application/json

{
  "statements": [{
    "statement": "MATCH (o:EngObject {tag: $tag}) RETURN o",
    "parameters": { "tag": "P-10101" }
  }]
}
```

The `Neo4jClient` in the Worker wraps `fetch()` calls to this endpoint. All existing Cypher queries (canonical patterns from `CLAUDE.md`) are unchanged.

#### 3d. Spatial index (R2 binding)

At cold start the Worker fetches `spatial_index.json` from R2 via the `TILEGRAPH_BUCKET` binding. The in-memory records are cached for the isolate lifetime. Subsequent requests reuse the cache; a new cold start (or manual re-deploy) re-fetches from R2.

```typescript
// Pseudo-code
const obj = await env.TILEGRAPH_BUCKET.get("tiles/index/spatial_index.json")
const data = await obj.json<SerializedIndex>()
```

#### 3e. Viewer bridge (Durable Objects)

The `ViewerHub` Durable Object replaces the local `ws://localhost:9001` server. It maintains WebSocket connections to all open viewer tabs and fans out `ViewerCommand` messages from tool handlers.

```
Viewer tab  ──WSS──▶  ViewerHub DO ◀──── Worker tool handler
                           │
                    (broadcasts to all
                     connected clients)
```

Worker routes WebSocket upgrade requests (`GET /ws/viewer`) to a single `ViewerHub` instance using a fixed stub ID, so all browser tabs and the Worker share the same hub.

#### 3f. Wrangler configuration

**`wrangler.toml`:**

```toml
name = "tilegraphmcp"
main = "src/worker.ts"
compatibility_date = "2024-12-01"
compatibility_flags = ["nodejs_compat"]

[[r2_buckets]]
binding = "TILEGRAPH_BUCKET"
bucket_name = "tilegraph-data"
```

**Secrets (set via `wrangler secret put`):**

| Secret              | Value                                   |
| ------------------- | --------------------------------------- |
| `NEO4J_URI`         | `neo4j+s://1c3578a5.databases.neo4j.io` |
| `NEO4J_USERNAME`    | `neo4j`                                 |
| `NEO4J_PASSWORD`    | Aura instance password                  |
| `NEO4J_DATABASE`    | `neo4j`                                 |
| `ANTHROPIC_API_KEY` | For the `/chat` agent loop              |

---

### 4. Cloudflare Pages — Viewer (`tilegraphviewer`)

A static single-page application (Vite + CesiumJS) hosted on Cloudflare Pages. All assets are served from Cloudflare's CDN edge.

**Pages project name:** `tilegraphviewer`

**Build:**

```bashs
cd apps/tilegraphviewer
npm run build   # → dist/
```

**Cloudflare Pages build settings:**

| Setting          | Value                  |
| ---------------- | ---------------------- |
| Framework preset | None (Vite)            |
| Build command    | `npm run build`        |
| Output directory | `dist`                 |
| Root directory   | `apps/tilegraphviewer` |
| Node.js version  | 20                     |

**Environment variables (Pages → Settings → Environment Variables):**

| Variable            | Value                                                                    |
| ------------------- | ------------------------------------------------------------------------ |
| `VITE_TILESET_PATH` | `https://pub-65db26f12b0942ce8e8a9d5cb5f36314.r2.dev/tiles/tileset.json` |
| `VITE_MCP_REST_URL` | `https://tilegraphmcp.quatricmorph.workers.dev`                          |
| `VITE_WS_URL`       | `wss://tilegraphmcp.quatricmorph.workers.dev/ws/viewer`                  |

s
The viewer's three runtime dependencies are all cloud-hosted:

| Dependency                            | Protocol    | Source                             |
| ------------------------------------- | ----------- | ---------------------------------- |
| `tileset.json` + `*.glb`              | HTTPS GET   | Cloudflare R2 (public bucket)      |
| `/hierarchy`, `/objects/:id`, `/chat` | HTTPS + SSE | Cloudflare Worker                  |
| Viewer commands (highlight, isolate)  | WSS         | Cloudflare Worker → Durable Object |

---

### 5. Neo4j Aura — Graph Database

**Instance:** `1c3578a5.databases.neo4j.io` (Neo4j Aura Free/Professional)

**Connection (Cloudflare Worker):** HTTPS transactional API on port 7473  
**Connection (Rust pipeline):** HTTP transactional API via `tilegraph-graph-export` `Neo4jClient`

**Graph model:**

All nodes carry `:EngObject` plus a class label (`:Pump`, `:Valve`, `:Line`, `:Area`, `:System`, etc.).

Key properties on every node:

| Property         | Description                                |
| ---------------- | ------------------------------------------ |
| `object_id`      | Deterministic SHA-256 UUID (`obj_<32hex>`) |
| `tag`            | Engineering tag (e.g. `P-10101`)           |
| `class`          | Object class string                        |
| `status`         | Operational status                         |
| `tile_id`        | 3D Tiles tile reference                    |
| `feature_id`     | GLB EXT_mesh_features feature ID           |
| `aabb_min_x/y/z` | Bounding box minimum                       |
| `aabb_max_x/y/z` | Bounding box maximum                       |

**Relationships:**

| Relationship   | Meaning                             |
| -------------- | ----------------------------------- |
| `PART_OF`      | Component → parent system/area      |
| `CONNECTED_TO` | Physical connection (piping, cable) |
| `UPSTREAM_OF`  | Flow direction                      |
| `ISOLATED_BY`  | Isolation valve → line              |
| `LOCATED_IN`   | Object → spatial area               |

**Population:** The Rust pipeline's `build-graph --push-to-neo4j` command runs `MERGE` Cypher over all nodes and relationships. This is idempotent — re-running after a pipeline change updates properties in place without duplicating nodes.

---

## End-to-end data flow

```
1. Rust pipeline runs locally
   ├── Generates/ingests plant objects
   ├── Builds 3D Tiles (GLBs + tileset.json)
   ├── Builds spatial index (spatial_index.json)
   └── Pushes graph to Neo4j Aura via HTTP API

2. Developer uploads outputs to R2
   └── tiles/, metadata/, index/ → tilegraph-data bucket

3. Browser loads tilegraphviewer (Cloudflare Pages)
   ├── CesiumJS fetches tileset.json from R2
   ├── CesiumJS streams *.glb tiles from R2 (LOD-based)
   └── JS connects WebSocket to Worker → ViewerHub DO

4. User types query in agent chat panel
   └── POST /chat → Cloudflare Worker

5. Worker runs Claude agent loop
   ├── Claude calls search_object_by_tag → Worker queries Neo4j Aura
   ├── Claude calls query_upstream_downstream → Worker queries Neo4j Aura
   ├── Claude calls highlight_objects_in_viewer → Worker sends WS command via ViewerHub DO
   └── SSE chunks stream back to browser as agent narrates

6. Viewer receives WS command from ViewerHub DO
   └── CesiumJS style updated: matching objects highlighted in 3D scene
```

---

## Local development setup

For end-to-end local development without deploying to Cloudflare:

```bash
# Terminal 1: Run local Neo4j via Docker (for dev only; prod uses Aura)
docker-compose up -d neo4j

# Terminal 2: Run Rust pipeline
cargo run --bin tilegraph -- generate-synth
cargo run --bin tilegraph -- build-tiles
cargo run --bin tilegraph -- build-graph --push-to-neo4j

# Terminal 3: Run MCP server locally (Wrangler dev mode)
cd apps/tilegraphmcp
npm run dev   # wrangler dev on :9000, with miniflare R2 simulation

# Terminal 4: Run viewer
cd apps/tilegraphviewer
npm run dev   # Vite on :5173
```

Local `.dev.vars` for the Worker:

```env
NEO4J_URI=bolt://localhost:7687
NEO4J_USERNAME=neo4j
NEO4J_PASSWORD=password
NEO4J_DATABASE=neo4j
ANTHROPIC_API_KEY=sk-ant-...
```

Viewer `.env.local`:

```env
VITE_TILESET_PATH=../../output/tiles/tileset.json
VITE_MCP_REST_URL=http://localhost:9000
VITE_WS_URL=ws://localhost:9000/ws/viewer
```

---

## CI/CD

### Automatic deploy on push (GitHub Actions)

```yaml
# .github/workflows/deploy.yml
on:
  push:
    branches: [main]

jobs:
  deploy-worker:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 20 }
      - run: npm ci
        working-directory: apps/tilegraphmcp
      - run: npm run build
        working-directory: apps/tilegraphmcp
      - uses: cloudflare/wrangler-action@v3
        with:
          apiToken: ${{ secrets.CF_API_TOKEN }}
          workingDirectory: apps/tilegraphmcp

  deploy-pages:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 20 }
      - run: npm ci
        working-directory: apps/tilegraphviewer
      - run: npm run build
        working-directory: apps/tilegraphviewer
      - uses: cloudflare/wrangler-action@v3
        with:
          apiToken: ${{ secrets.CF_API_TOKEN }}
          command: pages deploy dist --project-name tilegraphviewer
          workingDirectory: apps/tilegraphviewer
```

Cloudflare Pages can also be connected directly to the GitHub repo for zero-config automatic deploys (recommended for the viewer).

---

## Security

| Concern           | Mitigation                                                                     |
| ----------------- | ------------------------------------------------------------------------------ |
| Neo4j credentials | Wrangler secrets (never in `wrangler.toml` or source)                          |
| Anthropic API key | Wrangler secret                                                                |
| R2 tile data      | Public read is intentional (3D Tiles require direct browser fetch); no PII     |
| Worker auth       | Optional: add Cloudflare Access in front of `/chat` and MCP endpoints          |
| CORS              | R2 CORS policy + Worker `Access-Control-Allow-Origin` headers restrict origins |

---

## Component summary

| Component              | Technology                                      | Hosting         |
| ---------------------- | ----------------------------------------------- | --------------- |
| Data pipeline          | Rust (cargo workpace)                           | Local           |
| Graph database         | Neo4j Aura                                      | Cloud (managed) |
| Tile + spatial data    | Cloudflare R2                                   | Cloudflare      |
| MCP / REST / WS server | Hono.js on Cloudflare Workers + Durable Objects | Cloudflare      |
| 3D viewer SPA          | CesiumJS + Vite on Cloudflare Pages             | Cloudflare      |
