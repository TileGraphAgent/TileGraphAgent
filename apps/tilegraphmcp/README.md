# TileGraphMCP

MCP (Model Context Protocol) server for the TileGraphAgent system, deployed as a **Cloudflare Worker** using [Hono.js](https://hono.dev). It connects to **Neo4j Aura** (cloud) for graph queries and reads spatial index data from **Cloudflare R2**.

## Architecture

```
Claude / AI Agent
     │  MCP over HTTP-SSE
     ▼
Cloudflare Worker (Hono.js)
     ├── /sse          ← MCP transport endpoint
     ├── /messages     ← MCP message handler
     ├── /chat         ← REST streaming (SSE) for viewer
     ├── /objects/:id  ← REST: object properties
     ├── /hierarchy    ← REST: area/system/line tree
     ├── /health       ← health check
     └── /ws/*         ← Durable Object WebSocket hub
          │                   │
          ▼                   ▼
   Neo4j Aura          Cloudflare R2
  (graph queries)   (spatial_index.json)
```

## Tech stack

| Concern        | Technology                                                  |
| -------------- | ----------------------------------------------------------- |
| Runtime        | Cloudflare Workers                                          |
| HTTP framework | Hono.js                                                     |
| MCP transport  | HTTP + SSE (`@modelcontextprotocol/sdk` SSEServerTransport) |
| Graph database | Neo4j Aura (`neo4j+s://`)                                   |
| Spatial index  | Cloudflare R2 (fetched at request time)                     |
| Viewer bridge  | Cloudflare Durable Objects (WebSocket hub)                  |
| Audit log      | Cloudflare R2 (append JSONL)                                |

## Local development

```bash
cd apps/tilegraphmcp
npm install
npm run dev      # wrangler dev (hot reload on localhost:9000)
```

For local dev, Wrangler proxies R2 bindings via miniflare. Set environment variables in `.dev.vars`:

```env
NEO4J_URI=neo4j+s://1c3578a5.databases.neo4j.io
NEO4J_USERNAME=neo4j
NEO4J_PASSWORD=<password>
NEO4J_DATABASE=neo4j
```

## Deployment

```bash
# Publish to Cloudflare Workers
npx wrangler deploy

# Set secrets (never in wrangler.toml)
npx wrangler secret put NEO4J_URI
npx wrangler secret put NEO4J_USERNAME
npx wrangler secret put NEO4J_PASSWORD
```

## wrangler.toml

```toml
name = "tilegraphmcp"
main = "src/worker.ts"
compatibility_date = "2024-12-01"
compatibility_flags = ["nodejs_compat"]

[[r2_buckets]]
binding = "TILEGRAPH_BUCKET"
bucket_name = "tilegraph-data"
```

## Environment variables

| Variable           | Description                                                       |
| ------------------ | ----------------------------------------------------------------- |
| `NEO4J_URI`        | Aura connection URI, e.g. `neo4j+s://1c3578a5.databases.neo4j.io` |
| `NEO4J_USERNAME`   | Neo4j username (default `neo4j`)                                  |
| `NEO4J_PASSWORD`   | Neo4j password (Wrangler secret)                                  |
| `NEO4J_DATABASE`   | Database name (default `neo4j`)                                   |
| `TILEGRAPH_BUCKET` | R2 binding — serves `spatial_index.json` and audit logs           |
| `VIEWER_HUB`       | Durable Object binding for WebSocket viewer bridge                |

## MCP tools exposed

| Tool                           | Description                                              |
| ------------------------------ | -------------------------------------------------------- |
| `search_object_by_tag`         | Resolve engineering tag → `object_id` (Neo4j)            |
| `get_object_properties`        | Full property set for an object (Neo4j)                  |
| `query_connected_components`   | Immediate neighbors via `CONNECTED_TO`/`PART_OF` (Neo4j) |
| `query_upstream_downstream`    | Flow path traversal up to N hops (Neo4j)                 |
| `query_nearby_objects`         | Radius search around a point (R2 spatial index)          |
| `query_objects_in_area`        | All objects under an area node (Neo4j)                   |
| `get_tile_feature_mapping`     | Confirm tile + feature_id for 3D geometry (Neo4j)        |
| `highlight_objects_in_viewer`  | Send highlight command to viewer (Durable Object WS)     |
| `isolate_system_in_viewer`     | Send isolate command to viewer (Durable Object WS)       |
| `focus_camera_on_objects`      | Send camera focus to viewer (Durable Object WS)          |
| `generate_maintenance_context` | Aggregate maintenance data for a line (Neo4j)            |
| `create_issue_from_selection`  | Log an engineering issue marker (R2 audit)               |

## Neo4j connection notes

The Worker uses **Neo4j's HTTP Transactional API** (`/db/{database}/tx/commit`) over HTTPS because the standard Bolt/WebSocket driver requires Node.js primitives unavailable in the Workers edge runtime. All Cypher queries are sent as `application/json` POST requests with Basic auth.

```
POST https://1c3578a5.databases.neo4j.io:7473/db/neo4j/tx/commit
Authorization: Basic base64(neo4j:<password>)
Content-Type: application/json

{ "statements": [{ "statement": "MATCH (o:EngObject {tag: $tag}) RETURN o", "parameters": { "tag": "P-10101" } }] }
```

## Viewer WebSocket bridge (Durable Objects)

The `ViewerHub` Durable Object maintains a persistent WebSocket session per viewer client. When a MCP tool calls `highlight_objects_in_viewer`, the Worker routes the command through the Durable Object to all connected viewer tabs.

```
Viewer tab  ──WS──▶  ViewerHub DO  ◀──── Worker (tool handler)
```

This replaces the local `ws://localhost:9001` bridge from the non-Worker version.

## Spatial index

At startup (or lazily per request), the Worker fetches `spatial_index.json` from R2:

```
R2 bucket: tilegraph-data
Key: tiles/index/spatial_index.json
```

The file is cached in-memory for the lifetime of the Worker isolate. It is re-fetched when the R2 object's `ETag` changes.

## Building and testing

```bash
npm run build    # tsc → dist/ (used by wrangler)
npm run test     # vitest run (unit tests)
npx wrangler dev # local Worker with miniflare bindings
```
