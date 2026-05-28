# TileGraphAgent

**TileGraphAgent turns industrial 3D from a visual asset into an agent-readable engineering system.**

Industrial CAD → 3D Tiles 1.1 → Knowledge Graph → MCP Agent Bridge

---

## Architecture

```
Synthetic Plant Spec (plant_spec.json)
    ↓ [tilegraph-synth]
Normalized Industrial Scene Graph
    ↓ [tilegraph-ingest / tilegraph-geometry]
Mesh + Metadata Split → Tessellated Mesh Groups
    ↓ [tilegraph-gltf]
GLB Content Files (area-a-piping.glb, area-a-equipment.glb, ...)
    ↓ [tilegraph-tiles]
3D Tiles 1.1 Tileset (tileset.json + metadata/)
    ↓
Spatial Index (R-tree / spatial_index.json)
    ↓ [tilegraph-graph-export]
Neo4j Knowledge Graph (EngObject nodes + relationships)
    ↓ [tilegraph-mcp-server]
MCP Server (12 tools + resources + audit log)
    ↓
LLM Agent → CesiumJS Viewer (WebSocket bridge)
```

---

## Quick Start

```bash
# 1. Start Neo4j
docker-compose up -d neo4j

# 2. Generate synthetic plant data
cargo run --bin tilegraph -- generate-synth

# 3. Build 3D Tiles + GLB content
cargo run --bin tilegraph -- build-tiles

# 4. Export Knowledge Graph
cargo run --bin tilegraph -- build-graph

# 5. Import to Neo4j
cat output/graph/schema.cypher output/graph/import.cypher | \
  docker exec -i tilegraph-agent-neo4j-1 cypher-shell -u neo4j -p password

# 6. Start MCP server
cd apps/tilegraph-mcp-server && npm install && npm run dev

# 7. Start viewer
cd apps/tilegraph-viewer && npm install && npm run dev

# 8. Validate pipeline
cargo run --bin tilegraph -- validate
```

---

## Rust Workspace Crates

| Crate | Purpose |
|-------|---------|
| `tilegraph-core` | Domain model, ObjectId, AABB, transforms, error types |
| `tilegraph-synth` | Synthetic industrial plant generator |
| `tilegraph-ingest` | Source adapters (synth + IFC stub) |
| `tilegraph-geometry` | Mesh tessellation, material library, geometry batching |
| `tilegraph-gltf` | GLB export with EXT_mesh_features feature IDs |
| `tilegraph-tiles` | 3D Tiles 1.1 tileset.json generation |
| `tilegraph-spatial` | R-tree spatial index (rstar crate) |
| `tilegraph-graph-export` | Neo4j Cypher + CSV export |
| `tilegraph-cli` | CLI entry point |

---

## MCP Tools (12)

`search_object_by_tag` · `get_object_properties` · `query_connected_components` · `query_upstream_downstream` · `query_objects_in_area` · `query_nearby_objects` · `get_tile_feature_mapping` · `highlight_objects_in_viewer` · `isolate_system_in_viewer` · `focus_camera_on_objects` · `create_issue_from_selection` · `generate_maintenance_context`

---

## Demo Question

> "Find all pumps connected to LINE-1001, show their isolation valves, isolate the affected system in the viewer, and explain the maintenance impact."

See `docs/architecture/demo_scenario.md` for the full tool chain, sample queries, and expected output.

---

## Limitations (V1)

- Synthetic data only — no real RVM, NWD, or IFC files
- IFC adapter is a stub (`tilegraph-ingest/src/ifc_stub.rs`)
- Single-level tile hierarchy (no LOD)
- Draco compression not yet implemented

*Portfolio project by Thanh Hoang-Minh — 2026*
