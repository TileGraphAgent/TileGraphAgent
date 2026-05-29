Both builds clean. Everything is in place. Here's a summary of what was
implemented:

---

MCP Server changes:

- src/tools/index.ts — exported TOOLS array (was const, now export const)
- src/agent/claude_agent.ts — new module; runs the full Claude agent loop,
executes tool calls through existing handlers, streams text/tool-feedback via
onChunk, returns AgentTurn[]
- src/index.ts — added POST /chat endpoint that sets SSE headers, streams
chunks as data: {...}\n\n events, emits a done event with turn count and tool
call names

Viewer changes:

- src/agent/claude_client.ts — new SSE client that posts to /chat and parses
data: lines from the response stream
- src/main.ts — wired the Ask button and Enter key to sendAgentMessage; text
streams into the assistant div in real time; [N turns, tools: ...] meta line
appended on completion
- index.html — added .msg-meta CSS style

Integration test:

- tests/integration/mock_neo4j.ts — in-memory Neo4j stub returning fixed data
for LINE-1001
- tests/integration/tool_chain.test.ts — skips without ANTHROPIC_API_KEY; spies
on all tool handlers to capture call order; asserts search_object_by_tag
precedes graph tools and get_tile_feature_mapping precedes viewer tools
















---
Stage 4.1 — Neo4j connection pooling & health check
- src/config.ts (new) — centralises env-based constants
(NEO4J_CONNECTION_TIMEOUT_MS, REST_PORT, etc.)
- src/db/neo4j.ts — driver now configured with maxConnectionPoolSize: 10 +
connectionAcquisitionTimeout; query() accepts an optional timeoutMs param and
races against a timeout promise; ServiceUnavailable errors are re-thrown with
error_code: "GRAPH_UNAVAILABLE"; healthCheck() passes 2 s timeout
- src/index.ts — imports constants from config.ts; logs Neo4j health at startup
(connected/unavailable)

Stage 4.2 — Input validation hardening
- src/schemas/validation.ts (new) — TagSchema, ObjectIdSchema (exact obj_<32 
hex>), ObjectIdArraySchema (max 50), RadiusSchema (max 500 m), DirectionSchema
- All 12 tool files — replaced bare z.string() / z.array(z.string()) with the
validated schemas
- src/tools/index.ts — catch block now distinguishes ZodError (→
VALIDATION_ERROR with per-field messages) from GRAPH_UNAVAILABLE vs generic
INTERNAL_ERROR

Stage 4.3 — WebSocket heartbeat & command queue
- src/viewer/bridge.ts — clients tracked by ID in a Map; 30 s heartbeat pings
every client; pong-timeout terminates stale connections; commandQueue (last 10)
replayed to new connections; primary-client tracking
- apps/tilegraph-viewer/src/agent/ws_client.ts — responds to ping with pong
before the command switch

Stage 4.4 — Audit log persistence & session queries
- src/audit/logger.ts — added callCount/totalDurationMs metrics;
rotateIfNeeded() at 10 MB; getSessionEntries(), getLastEntries(),
getSessionSummary(); session ID now includes a random suffix to prevent
10 MB; getSessionEntries(), getLastEntries(), getSessionSummary(); session ID now
includes a random suffix to prevent millisecond collisions
- src/resources/index.ts — exposes tilegraph://audit/session/{id} and
tilegraph://audit/last/N MCP resources

Tests — 28 new vitest tests covering all validation schemas and audit logger
behaviour.














MCP Server changes:
  
  - src/tools/index.ts — exported TOOLS array (was const, now export const)
  - src/agent/claude_agent.ts — new module; runs the full Claude agent loop,
  executes tool calls through existing handlers, streams text/tool-feedback via
  onChunk, returns AgentTurn[]
  - src/index.ts — added POST /chat endpoint that sets SSE headers, streams
  chunks as data: {...}\n\n events, emits a done event with turn count and tool
  call names

  Viewer changes:

  - src/agent/claude_client.ts — new SSE client that posts to /chat and parses
  data: lines from the response stream
  - src/main.ts — wired the Ask button and Enter key to sendAgentMessage; text
  streams into the assistant div in real time; [N turns, tools: ...] meta line
  appended on completion
  - index.html — added .msg-meta CSS style

  Integration test:

  - tests/integration/mock_neo4j.ts — in-memory Neo4j stub returning fixed data for
  LINE-1001
  - tests/integration/tool_chain.test.ts — skips without ANTHROPIC_API_KEY; spies on
  all tool handlers to capture call order; asserts search_object_by_tag precedes graph
  tools and get_tile_feature_mapping precedes viewer tools
















Stage 2.2 — 3-Level LOD Hierarchy

- crates/tilegraph-tiles/src/lod.rs (new) — LodLevel enum (Lod0/1/2), LodStrategy
trait, ClassBasedLod implementation: Tank/Equipment → LOD0, Pump/Valve/Instrument →
LOD1, PipeSegment/Support/Flange/CableTray/Nozzle/AccessPlatform → LOD2.
- geometric_error.rs — lod_geometric_error(): LOD0 = max(d×0.5, 50m), LOD1 =
max(d×0.08, 5m), LOD2 = max(d×0.01, 0.5m).
- builder.rs — Replaced AreaBatch-based flat 2-level tree with LodBatch-based
4-level tree: root → area → sector → cell → content leaf. AreaBatch kept for
backward compat.
- build_tiles.rs — Objects per area split into 3 LOD groups; each group produces its
own GeometryGroup with batch IDs like area-a-lod0-equipment, creating 12 LOD-tagged
GLBs for the synthetic plant (no second geometry pass needed).

Stage 2.3 — Mesh Instancing

- instance.rs — InstanceKey, updated InstanceGroup/InstanceRecord (raw TRS arrays),
build_instance_groups() — groups Support/Flange objects sharing the same
class+nominal-bore when ≥3 instances exist.
- schema.rs — Node gets an extensions: Option<serde_json::Value> field for
EXT_mesh_gpu_instancing.
- builder.rs — add_mesh_geometry() (mesh data without node), add_instance_group() —
packs TRANSLATION/ROTATION/SCALE/_FEATURE_ID_0 per-instance accessors and emits the
node extension.
- writer.rs — write_batch_instanced() separates Support/Flange meshes, builds
instance groups, falls back to individual meshes for groups < 3.

Results: area-a-lod2-support.glb and area-b-lod2-support.glb both carry
EXT_mesh_gpu_instancing; validate reports passed: true; tileset depth is 4
(root→area→sector→cell→content).











Stage 2.2 — 3-Level LOD Hierarchy

- crates/tilegraph-tiles/src/lod.rs (new) — LodLevel enum (Lod0/1/2), LodStrategy
trait, ClassBasedLod implementation: Tank/Equipment → LOD0, Pump/Valve/Instrument →
LOD1, PipeSegment/Support/Flange/CableTray/Nozzle/AccessPlatform → LOD2.
- geometric_error.rs — lod_geometric_error(): LOD0 = max(d×0.5, 50m), LOD1 =
max(d×0.08, 5m), LOD2 = max(d×0.01, 0.5m).
- builder.rs — Replaced AreaBatch-based flat 2-level tree with LodBatch-based
4-level tree: root → area → sector → cell → content leaf. AreaBatch kept for
backward compat.
- build_tiles.rs — Objects per area split into 3 LOD groups; each group produces its
own GeometryGroup with batch IDs like area-a-lod0-equipment, creating 12 LOD-tagged
GLBs for the synthetic plant (no second geometry pass needed).

Stage 2.3 — Mesh Instancing

- instance.rs — InstanceKey, updated InstanceGroup/InstanceRecord (raw TRS arrays),
build_instance_groups() — groups Support/Flange objects sharing the same
class+nominal-bore when ≥3 instances exist.
- schema.rs — Node gets an extensions: Option<serde_json::Value> field for
EXT_mesh_gpu_instancing.
- builder.rs — add_mesh_geometry() (mesh data without node), add_instance_group() —
packs TRANSLATION/ROTATION/SCALE/_FEATURE_ID_0 per-instance accessors and emits the
node extension.
- writer.rs — write_batch_instanced() separates Support/Flange meshes, builds
instance groups, falls back to individual meshes for groups < 3.

Results: area-a-lod2-support.glb and area-b-lod2-support.glb both carry
EXT_mesh_gpu_instancing; validate reports passed: true; tileset depth is 4
(root→area→sector→cell→content).


