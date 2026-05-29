You are a principal industrial 3D platform architect, CAD/BIM pipeline engineer, Rust/C++ systems engineer, 3D Tiles 1.1 specialist, glTF/GLB optimization engineer, CesiumJS engineer, graph database architect, and AI-agent/MCP platform engineer.

I am preparing a serious portfolio project for the role:

# Industrial 3D × AI Agent Platform Engineer

## Agentic 3D Platform Engineer — Industrial CAD × AI

The company context is:

- LAPIS3D focuses on bridging Large Language Models and 3D spatial data for EPC and shipbuilding industries.
- Their stack includes industrial 3D data pipelines, optimized 3D Tiles output, web-based industrial 3D viewers, component/mesh instancing, model tree navigation, property queries, clipping, viewpoints, issue tracking, measurements, and APIs for item search/color settings/custom workflows.
- Intratech3D focuses on engineering data interoperability, 3D CAD data conversion, P&ID extraction, change management, and custom engineering software for EPC, heavy industry, BIM, and CAD workflows.

The target JD section is:

```text
What you will do

3D pipeline evolution — design and ship the next-generation RVM / NWD / IFC → 3D Tiles pipeline in C++ / Rust

Agent Core — build the bridging logic between LLMs, MCP Servers, and the Knowledge Graph, exposing 3D Tiles data so agents can reason and act on it

Use AI tools heavily in your own development workflow — AI-assisted development is a required skill, not a perk
```

Your task is to design a complete first project named:

# TileGraphAgent

## Industrial CAD → 3D Tiles 1.1 → Knowledge Graph → MCP Agent Bridge

This is not a toy 3D viewer project.

This is a serious industrial 3D × AI portfolio project that demonstrates I can contribute to a production stack involving:

- RVM / NWD / IFC-style industrial CAD/BIM ingestion
- 3D Tiles 1.1 pipeline design
- glTF/GLB generation and optimization
- BVH / R-tree / LOD / spatial indexing
- industrial object identity preservation
- Knowledge Graph conversion
- MCP Server tools/resources
- LLM agent reasoning over 3D spatial + engineering semantic data
- CesiumJS viewer integration
- AI-assisted development workflow with manual verification

Because I may not have access to proprietary RVM, NWD, NWC, S3D, or SP3D data, design the project around a legal and reproducible V1 dataset strategy:

- generated synthetic EPC plant model
- optional open IFC sample model support
- mock industrial equipment:
  - pumps
  - tanks
  - valves
  - pipe segments
  - elbows
  - flanges
  - nozzles
  - instruments
  - supports
  - cable trays
  - access platforms

- synthetic engineering tags:
  - `P-1001`
  - `V-1001A`
  - `LINE-1001`
  - `TANK-TK-2001`
  - `PID-AREA-A-001`

- synthetic P&ID-like relationship tables
- synthetic datasheet JSON files
- synthetic work packages and issue records

The architecture must still be designed so that future adapters can support:

- RVM
- NWD / NWC
- IFC
- DWG metadata extraction
- Smart3D / SP3D MDB-like engineering tables
- P&ID linking
- datasheet linking
- revision/change comparison

Use the following concrete technical choices unless you have a strong reason to improve them:

## Core technical choices

### Language choices

- Use Rust as the main systems language for the pipeline.
- Use TypeScript for the MCP Server and CesiumJS viewer.
- Use Python only for optional dataset generation scripts or quick validation notebooks.
- Mention where C++ would be used in a production-grade version, especially for native CAD SDK integration, geometry kernels, or proprietary RVM/NWD readers.

### Rust workspace crates

Design the Rust workspace with these crates:

- `tilegraph-core`
  - shared domain model
  - object IDs
  - transform math
  - AABB / bounding volumes
  - error types
  - serialization contracts

- `tilegraph-synth`
  - synthetic industrial plant generator
  - procedural geometry primitives
  - mock engineering metadata

- `tilegraph-ingest`
  - source adapters
  - V1: synthetic JSON scene
  - V2: IFC adapter placeholder
  - future: RVM/NWD/NWC adapter interface

- `tilegraph-geometry`
  - mesh generation
  - primitive-to-mesh conversion
  - transform flattening
  - instancing preparation
  - mesh grouping by material/object class

- `tilegraph-gltf`
  - GLB export
  - node/object mapping
  - feature ID mapping
  - metadata attachment strategy
  - optional Draco/meshopt notes as future work

- `tilegraph-tiles`
  - `tileset.json` generation
  - tile hierarchy
  - bounding volume calculation
  - geometric error estimation
  - content URI generation
  - 3D Tiles 1.1 metadata strategy

- `tilegraph-spatial`
  - R-tree spatial index using Rust `rstar`
  - optional BVH implementation
  - bounding-box query
  - nearest-neighbor query
  - object-to-tile lookup

- `tilegraph-graph-export`
  - Neo4j Cypher export
  - CSV export for nodes/relationships
  - object identity mapping
  - graph validation

- `tilegraph-cli`
  - command-line interface:
    - `tilegraph generate-synth`
    - `tilegraph build-tiles`
    - `tilegraph build-graph`
    - `tilegraph validate`
    - `tilegraph inspect-object`
    - `tilegraph benchmark`

### TypeScript services

Use TypeScript for:

- `apps/tilegraphmcp`
  - MCP Server exposing tools and resources
  - schema validation with Zod
  - Neo4j queries
  - spatial index lookup through local JSON/Rust-generated index files or HTTP bridge
  - audit logging
  - viewer event bridge

- `apps/tilegraph-viewer`
  - CesiumJS viewer
  - load generated 3D Tiles tileset
  - object selection
  - property panel
  - tag search
  - agent action panel
  - highlight/isolate objects
  - audit trail panel

### Database choices

Use Neo4j for V1 because Cypher is readable and good for portfolio explanation.

Use these graph design rules:

- Every engineering object must have a stable `object_id`.
- Every tag must be unique when applicable.
- Every visual feature must map back to an engineering object.
- Every graph object that has geometry must map to:
  - tile ID
  - glTF node index or feature ID
  - bounding volume
  - source adapter reference

- Relationships must distinguish:
  - physical containment
  - engineering connectivity
  - document linkage
  - spatial proximity
  - operational dependency

### Spatial index choices

Use:

- R-tree for V1 object-level spatial query.
- Optional custom BVH implementation for portfolio depth.
- AABB as the default object bounding volume.
- Oriented bounding box as future work.
- Use millimeters or meters consistently and document the unit policy.

### Viewer communication choices

Use:

- WebSocket for live viewer commands from the MCP server:
  - highlight objects
  - isolate system
  - focus camera
  - show bounding boxes
  - create issue marker

- REST endpoints for:
  - object property lookup
  - search by tag
  - current selection state

- A small event bus model:
  - `ObjectSelected`
  - `AgentHighlightRequested`
  - `SystemIsolated`
  - `IssueCreated`
  - `AuditLogUpdated`

### Agent design choices

The LLM agent must never hallucinate engineering facts.

It must use deterministic MCP tools to:

1. search object by tag
2. retrieve object properties
3. query graph connectivity
4. query upstream/downstream relationships
5. query spatial objects in area
6. map graph objects to 3D Tiles feature IDs
7. command viewer highlight/isolation
8. generate maintenance context from structured facts
9. return an answer with evidence and uncertainty

The agent must not directly manipulate the viewer or graph database except through approved MCP tools.

The MCP Server must produce audit logs for every tool call.

---

# Required output

Generate a practical, technical, implementation-ready project plan for TileGraphAgent.

The answer must be specific enough that I can directly start coding after reading it.

Do not write vague descriptions.

Do not say “use a database” — specify Neo4j labels, properties, indexes, constraints, and example Cypher.

Do not say “build a pipeline” — specify Rust crates, data formats, CLI commands, intermediate files, and validation steps.

Do not say “integrate with MCP” — specify MCP tools, input schemas, output schemas, safety rules, and example tool calls.

Do not say “use 3D Tiles” — specify tileset structure, GLB content layout, metadata mapping, bounding volumes, geometric error, and object ID strategy.

---

# Sections to generate

## 1. Problem framing

Explain the real engineering problem behind TileGraphAgent.

Focus on:

- industrial 3D is not game 3D
- object identity is more important than visual mesh
- CAD/BIM data is messy, proprietary, heavy, and semantically rich
- LLM agents cannot reason from triangles alone
- 3D Tiles, spatial index, and Knowledge Graph must share a stable identity layer
- MCP is useful only if the tools are deterministic, schema-bound, and auditable
- the project must demonstrate pipeline engineering, not only UI

Explain the core thesis:

> TileGraphAgent turns industrial 3D from a visual asset into an agent-readable engineering system.

## 2. Project positioning for the JD

Map TileGraphAgent directly to the JD.

Create a table with columns:

- JD requirement
- TileGraphAgent feature
- Evidence in the repository
- Interview talking point

Cover:

- RVM / NWD / IFC → 3D Tiles thinking
- C++ / Rust pipeline design
- 3D Tiles 1.1
- glTF/GLB
- BVH / LOD / spatial indexing
- CAD/BIM data structure awareness
- Knowledge Graph
- MCP Server
- LLM agent workflow
- AI-assisted development
- correctness/debugging discipline

## 3. Final demo scenario

Design the final demo in detail.

Use this exact demo question:

> “Find all pumps connected to LINE-1001, show their isolation valves, isolate the affected system in the viewer, and explain the maintenance impact.”

Show the full flow:

1. User asks question in agent panel.
2. Agent parses intent.
3. Agent calls `search_object_by_tag`.
4. Agent calls `query_connected_components`.
5. Agent calls `query_upstream_downstream`.
6. Agent calls `get_tile_feature_mapping`.
7. Agent calls `isolate_system_in_viewer`.
8. Agent calls `highlight_objects_in_viewer`.
9. Agent calls `generate_maintenance_context`.
10. Viewer highlights pumps, valves, and line segments.
11. Agent returns final answer with structured evidence.
12. Audit log records every step.

Include:

- sample MCP tool calls
- sample JSON outputs
- sample graph query
- sample viewer command
- final agent response
- audit log example

## 4. Repository architecture

Propose a full monorepo structure.

Use this style:

```text
tilegraph-agent/
  README.md
  Cargo.toml
  package.json
  docker-compose.yml
  crates/
    tilegraph-core/
    tilegraph-synth/
    tilegraph-ingest/
    tilegraph-geometry/
    tilegraph-gltf/
    tilegraph-tiles/
    tilegraph-spatial/
    tilegraph-graph-export/
    tilegraph-cli/
  apps/
    tilegraphmcp/
    tilegraph-viewer/
  data/
    synth/
    ifc/
    metadata/
    pid/
    datasheets/
  output/
    tiles/
    graph/
    index/
    reports/
  docs/
    adr/
    architecture/
    pipeline/
    graph/
    mcp/
    viewer/
    ai-workflow/
  scripts/
  tests/
  benchmarks/
```

For each folder, explain its purpose.

## 5. End-to-end data flow

Design the full data flow.

Use this pipeline:

```text
Synthetic Plant Spec / IFC Sample
→ Normalized Industrial Scene Graph
→ Geometry + Metadata Split
→ Mesh/Instance Groups
→ GLB Content
→ 3D Tiles Tileset
→ Spatial Index
→ Knowledge Graph Export
→ MCP Server
→ Agent Workflow
→ CesiumJS Viewer Actions
```

For each stage, define:

- input
- output
- file format
- owner module
- validation rule
- possible failure mode

## 6. Domain data model

Design the minimal but extensible industrial data model.

Include these entities:

- Plant
- Area
- Unit
- System
- Line
- PipeSegment
- Valve
- Pump
- Tank
- Equipment
- Support
- CableTray
- Instrument
- Nozzle
- Flange
- Document
- PID
- Datasheet
- WorkPackage
- Issue
- Tile
- Feature
- BoundingVolume

For each entity, define:

- required fields
- optional fields
- example JSON
- graph label
- geometry mapping rule

Include stable ID strategy:

- `object_id`
- `tag`
- `source_id`
- `revision_id`
- `tile_id`
- `feature_id`
- `gltf_node`
- `global_transform`
- `aabb`

## 7. Relationship model

Define the relationships:

- `PART_OF`
- `LOCATED_IN`
- `CONNECTED_TO`
- `UPSTREAM_OF`
- `DOWNSTREAM_OF`
- `HAS_TAG`
- `HAS_DATASHEET`
- `APPEARS_IN_PID`
- `HAS_TILE_CONTENT`
- `HAS_FEATURE`
- `HAS_BOUNDING_VOLUME`
- `NEAR`
- `REQUIRES_ACCESS_CLEARANCE`
- `HAS_ISSUE`
- `AFFECTS`
- `ISOLATED_BY`

For each relationship, define:

- source node
- target node
- properties
- example
- why it matters for agent reasoning

## 8. Synthetic dataset generator

Design the V1 synthetic plant generator.

It should generate:

- 1 plant
- 2 areas
- 3 systems
- 5 lines
- 3 tanks
- 4 pumps
- 20 valves
- 80 pipe segments
- 12 instruments
- 40 supports
- 2 cable tray routes
- 5 P&ID mock documents
- 10 datasheets
- 5 maintenance work packages

Include:

- input config format
- output files
- procedural geometry strategy
- tag naming convention
- connection graph generation
- validation checks

Generate a sample `plant_spec.json`.

## 9. Rust implementation plan

For each Rust crate, define:

- responsibilities
- key structs
- key traits
- key functions
- dependencies
- tests
- CLI commands that use it

Include sample Rust struct definitions for:

- `ObjectId`
- `IndustrialObject`
- `ObjectClass`
- `Transform`
- `Aabb`
- `MeshPrimitive`
- `TileNode`
- `FeatureMapping`
- `GraphNodeExport`
- `SpatialIndexRecord`

Include sample trait definitions for:

- `SourceAdapter`
- `GeometryEmitter`
- `TileWriter`
- `SpatialIndex`
- `GraphExporter`

## 10. 3D Tiles 1.1 pipeline design

Design a simplified but credible 3D Tiles pipeline.

Include:

- coordinate system policy
- unit policy
- root transform policy
- object identity preservation
- mesh instancing strategy
- material strategy
- GLB content grouping
- tile hierarchy generation
- bounding volume calculation
- geometric error estimation
- LOD strategy
- feature/object ID mapping
- metadata attachment
- validation report

Specify output layout:

```text
output/tiles/
  tileset.json
  content/
    area-a-root.glb
    area-a-piping.glb
    area-a-equipment.glb
    area-b-root.glb
  metadata/
    feature_table.json
    object_properties.json
    tile_feature_map.json
  index/
    spatial_index.json
    spatial_index.bin
  reports/
    validation_report.json
```

Include example `tileset.json` snippet.

Include example `feature_table.json`.

Include example `tile_feature_map.json`.

Explain what is V1-compatible and what is future 3D Tiles 1.1 advanced metadata work.

## 11. GLB/glTF design

Specify how glTF nodes and meshes map to industrial objects.

Include:

- node naming convention
- mesh grouping
- instance grouping
- `extras` metadata strategy
- feature ID strategy
- material naming
- selection mapping
- limitations

Give example glTF node metadata:

```json
{
  "name": "PUMP-P-1001",
  "extras": {
    "object_id": "obj_pump_p_1001",
    "tag": "P-1001",
    "class": "Pump",
    "system": "SYS-COOLING",
    "line_refs": ["LINE-1001"],
    "feature_id": 1201
  }
}
```

## 12. Spatial indexing design

Design the object-level spatial index.

Include:

- AABB generation
- R-tree index
- optional BVH
- object lookup
- tile lookup
- bounding box query
- nearby query
- clearance/access query

Include example index record.

Include example query:

```text
Find all valves within 5 meters of PUMP-P-1001.
```

Show:

- input
- spatial query process
- output
- validation rule

## 13. Knowledge Graph design using Neo4j

Design a Neo4j schema.

Include:

- node labels
- properties
- uniqueness constraints
- indexes
- relationship types
- relationship properties

Write Cypher for:

- create constraints
- import object nodes
- import relationships
- find object by tag
- find all pumps connected to LINE-1001
- find isolation valves for LINE-1001
- find upstream/downstream objects
- find objects appearing in a P&ID
- find graph objects mapped to 3D Tiles features
- generate maintenance context for a line shutdown

Include example Cypher results.

## 14. MCP Server design

Design the MCP Server:

# `tilegraphmcp`

Use TypeScript.

Use schema validation with Zod.

Use Neo4j driver.

Use WebSocket bridge to the viewer.

Use local JSON index files for spatial mapping in V1.

Define these MCP tools:

1. `search_object_by_tag`
2. `get_object_properties`
3. `query_connected_components`
4. `query_upstream_downstream`
5. `query_objects_in_area`
6. `query_nearby_objects`
7. `get_tile_feature_mapping`
8. `highlight_objects_in_viewer`
9. `isolate_system_in_viewer`
10. `focus_camera_on_objects`
11. `create_issue_from_selection`
12. `generate_maintenance_context`

For each tool, provide:

- purpose
- input schema
- output schema
- safety rules
- example call
- example result
- failure cases

Define these MCP resources:

- `tilegraph://model/summary`
- `tilegraph://object/{tag}`
- `tilegraph://system/{system_id}`
- `tilegraph://line/{line_id}`
- `tilegraph://pid/{pid_id}`
- `tilegraph://selection/current`
- `tilegraph://audit/session/{session_id}`

For each resource, provide:

- URI pattern
- returned content type
- example output
- when the agent should read it

## 15. Agent system prompt

Write the actual system prompt for the TileGraphAgent LLM.

The agent must obey these rules:

- Never infer engineering facts without tool evidence.
- Always resolve tags to object IDs before reasoning.
- Always distinguish graph connectivity from spatial proximity.
- Never execute viewer actions without confirmed object mappings.
- Always include uncertainty when source data is synthetic, missing, or ambiguous.
- Always cite structured evidence from tool results.
- Never claim a shutdown/isolation is safe unless the graph explicitly supports it.
- Always produce an audit-friendly final answer.

Include:

- system prompt
- developer prompt
- tool-use policy
- refusal/fallback behavior
- confidence scoring rubric
- final answer template

## 16. Viewer integration design

Design the CesiumJS viewer.

Features:

- load generated `tileset.json`
- show model tree
- select object
- show properties panel
- search by tag
- highlight object list
- isolate system
- focus camera on object group
- show bounding boxes
- show connected components
- show P&ID/document links
- create issue marker
- show agent chat panel
- show audit trail panel

Explain the state model:

- selected object
- highlighted objects
- isolated system
- current agent session
- current audit log
- viewer command queue

Define WebSocket messages:

- `highlight_objects`
- `isolate_objects`
- `focus_camera`
- `show_bounding_boxes`
- `clear_highlights`
- `create_issue_marker`

Give example JSON messages.

## 17. Validation and correctness strategy

Design validation deeply.

Include validators for:

- object ID uniqueness
- tag uniqueness
- graph relationship consistency
- missing geometry
- missing graph node
- missing tile mapping
- invalid bounding volume
- disconnected line segments
- orphan valves
- P&ID reference mismatch
- datasheet reference mismatch
- feature ID mismatch
- viewer selection mismatch
- MCP output schema mismatch

Include:

- validation CLI command
- validation report JSON
- test cases
- manual verification checklist

## 18. Benchmarks

Design benchmarks that are meaningful for the JD.

Include:

- number of objects
- number of triangles
- number of GLB files
- tileset generation time
- graph export time
- spatial index build time
- tag query latency
- connected component query latency
- viewer highlight latency
- MCP tool latency
- end-to-end agent task latency

Create a benchmark table template.

## 19. AI-assisted development workflow

Design how I should use Claude Code / Cursor / GPT productively.

Include:

- how to use AI for architecture drafting
- how to use AI for Rust implementation
- how to use AI for test generation
- how to use AI for Cypher queries
- how to use AI for MCP schemas
- how to use AI for debugging
- how to review AI-generated code
- how to prevent fake correctness
- how to document AI-assisted commits

Create an `AI_VERIFICATION.md` structure.

Include examples of prompts for:

- implementing a Rust crate
- generating tests
- reviewing unsafe assumptions
- checking 3D Tiles schema
- verifying graph queries
- debugging viewer selection mismatch

## 20. Six-week implementation roadmap

Create a realistic six-week implementation roadmap.

For each week include:

- goals
- deliverables
- acceptance criteria
- risks
- AI-assisted workflow
- manual verification steps
- demo artifact

Use this structure:

### Week 1 — Domain model + synthetic plant generator

### Week 2 — Geometry + GLB export

### Week 3 — 3D Tiles + spatial index

### Week 4 — Neo4j Knowledge Graph

### Week 5 — MCP Server + agent workflow

### Week 6 — CesiumJS viewer + final demo polish

## 21. GitHub README outline

Write a strong README outline.

Include:

- project title
- one-sentence thesis
- architecture diagram placeholder
- demo video/GIF placeholder
- problem statement
- why industrial 3D is hard
- pipeline overview
- data model
- 3D Tiles output
- Knowledge Graph
- MCP Agent Bridge
- CesiumJS viewer
- how to run
- sample queries
- benchmark results
- validation report
- limitations
- future work
- relation to the LAPIS3D JD

## 22. Interview talking points

Generate practical interview talking points.

Focus on:

- why industrial 3D is different from game 3D
- why object identity is harder than rendering
- why source-to-feature mapping matters
- why 3D Tiles metadata matters
- why graph + spatial index + tiles must share IDs
- why MCP is a good bridge for LLM agents
- why agent actions must be deterministic and auditable
- how I used AI tools while manually verifying correctness
- what I would improve with real RVM/NWD/S3D data
- where Rust is useful
- where C++ would be necessary in production

## 23. Risks and limitations

Clearly list project risks:

- synthetic data may not capture real CAD messiness
- no real RVM/NWD reader in V1
- IFC support may be partial
- Cesium feature picking may need careful mapping
- GLB metadata may not fully match 3D Tiles 1.1 advanced metadata
- graph connectivity may oversimplify real piping/P&ID logic
- LLM agent may overstate engineering conclusions
- MCP server security and permission boundaries must be explicit

For each risk, provide mitigation.

## 24. Final deliverables checklist

Create a final checklist of what should exist in the repository after six weeks:

- working Rust CLI
- synthetic plant generator
- generated GLB content
- generated 3D Tiles tileset
- generated spatial index
- Neo4j graph import
- MCP Server
- CesiumJS viewer
- final demo script
- benchmark report
- validation report
- README
- architecture docs
- AI verification docs
- interview notes

## 25. Online research instruction

Before generating the final project plan, search online for the latest official or trustworthy information about:

- LAPIS3D
- Intratech3D
- 3D Tiles 1.1
- OGC 3D Tiles specification
- CesiumJS 3D Tiles implementation
- glTF 2.0
- glTF metadata / feature ID extensions
- Model Context Protocol
- MCP tools and resources
- Neo4j graph modeling
- ArangoDB as alternative
- IFC sample datasets
- open BIM sample models
- Rust libraries for glTF, geometry, spatial indexing, BVH, R-tree
- C++ geometry/CAD library options
- industrial CAD/BIM conversion references

Use only trustworthy sources:

- official websites
- official specifications
- official GitHub repositories
- OGC
- Cesium
- Khronos
- Model Context Protocol official documentation
- Neo4j documentation
- ArangoDB documentation
- buildingSMART / IFC references
- recognized engineering/CAD/BIM resources

Clearly separate:

- confirmed facts
- reasonable assumptions
- implementation suggestions
- risks / unknowns

Now generate the full project plan in a practical engineering style with concrete technical details, schemas, file structures, command examples, code snippets, and implementation priorities.
