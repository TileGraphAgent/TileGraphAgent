You are a senior product designer, B2B SaaS landing-page strategist, industrial software UX designer, technical copywriter, and frontend engineer.

Design and generate a high-end professional landing page for:

# TileGraphAgent

## Industrial CAD → 3D Tiles 1.1 → Knowledge Graph → MCP Agent Bridge

TileGraphAgent is a serious industrial 3D × AI platform project. It transforms heavy CAD/BIM-style industrial plant models into optimized 3D Tiles, preserves engineering object identity, exports semantic Knowledge Graph data, and exposes deterministic MCP tools so AI agents can reason over 3D spatial and engineering data safely.

This is not a game 3D viewer, not a simple WebGL demo, and not a generic AI chatbot.
It is an industrial-grade agentic 3D data platform for EPC, shipbuilding, plant engineering, BIM, digital twin, maintenance planning, and engineering data interoperability.

---

# 1. Landing page goal

Create a landing page that makes TileGraphAgent feel like a premium technical product for:

- Industrial 3D platform teams
- EPC software teams
- Shipbuilding engineering teams
- CAD/BIM pipeline engineers
- Digital twin teams
- AI-agent infrastructure teams
- Recruiters and engineering managers evaluating a serious portfolio project
- Companies working with RVM, NWD, IFC, 3D Tiles, CesiumJS, graph databases, and MCP

The page should communicate:

> TileGraphAgent turns industrial 3D from a visual asset into an agent-readable engineering system.

The landing page should immediately show that this is a deep technical platform involving:

- CAD/BIM-style ingestion
- 3D Tiles 1.1 generation
- glTF/GLB optimization
- object identity preservation
- spatial indexing
- Knowledge Graph export
- MCP Server tools/resources
- CesiumJS viewer integration
- deterministic AI-agent workflow
- audit-friendly engineering reasoning

---

# 2. Visual direction

Use a premium industrial AI aesthetic.

## Theme

Professional, technical, enterprise, industrial, trustworthy, precise.

Avoid:

- playful startup colors
- generic purple AI gradients
- childish 3D illustrations
- cartoon robots
- crypto/Web3 style
- overly futuristic sci-fi

The design should feel closer to:

- Cesium / Palantir / Hexagon / Bentley Systems / Siemens industrial software
- modern engineering dashboard
- Microsoft Fluent UI enterprise SaaS
- dark technical command-center UI
- high-end digital twin platform

## Color palette

Use a dark industrial base with technical accent colors.

Recommended palette:

- Background: `#07111F` deep navy-black
- Surface: `#0B1628` / `#101D33`
- Elevated panels: `#13243D`
- Primary accent: `#38BDF8` cyan-blue
- Secondary accent: `#22D3EE` electric cyan
- Graph accent: `#A3E635` lime-green
- Warning/engineering highlight: `#F59E0B` amber
- Text primary: `#F8FAFC`
- Text secondary: `#94A3B8`
- Border: `#24344D`
- Success: `#10B981`
- Error/risk: `#EF4444`

Use gradients subtly:

- dark navy → deep graphite
- cyan glow only for technical highlights
- avoid excessive neon

## Typography

Use a modern technical SaaS font stack:

```css
font-family:
  Segoe UI,
  system-ui,
  sans-serif;
```

Typography style:

- Large bold hero headline
- Tight technical subheadline
- Clear section headings
- Mono-style technical labels
- Small badge labels for pipeline stages

## UI style

Use:

- dark dashboard panels
- subtle glassmorphism, but not too blurry
- thin technical grid background
- 3D wireframe plant model silhouette
- node-link Knowledge Graph visualization
- pipeline flow diagrams
- code/schema cards
- audit log cards
- viewer command cards
- Cesium-like 3D viewport mockup

Use rounded corners, but not too soft:

- Cards: `16px`
- Buttons: `10px`
- Small chips: `999px`

Use subtle motion:

- pipeline nodes fade in
- graph nodes pulse gently
- hero 3D viewport has slow camera drift
- command logs slide upward
- agent tool calls animate step-by-step

---

# 3. Landing page structure

Generate a complete landing page with the following sections.

---

## Section 1 — Hero

Hero headline:

# Industrial 3D, made agent-readable.

Alternative headline options:

- Turn CAD/BIM geometry into an AI-agent reasoning layer.
- From 3D Tiles to Knowledge Graphs to deterministic MCP agents.
- The bridge between industrial 3D models and engineering AI agents.

Hero subheadline:

TileGraphAgent converts industrial CAD/BIM-style models into optimized 3D Tiles 1.1, preserves object identity, exports a Knowledge Graph, and exposes MCP tools so AI agents can query, reason, highlight, isolate, and explain engineering systems with auditable evidence.

Hero badges:

- Rust Pipeline
- 3D Tiles 1.1
- glTF / GLB
- Neo4j Knowledge Graph
- MCP Server
- CesiumJS Viewer
- Spatial Index
- Audit Logs

Primary CTA:

`View Architecture`

Secondary CTA:

`Watch Demo Flow`

Hero visual:

Create a split technical hero mockup:

Left side:

- 3D industrial plant viewport
- highlighted pipe line `LINE-1001`
- pumps `P-1001`, `P-1002`
- valves highlighted in amber
- floating labels with object IDs

Right side:

- agent tool-call panel showing:

```text
search_object_by_tag("LINE-1001")
query_connected_components(object_id)
get_tile_feature_mapping(objects)
isolate_system_in_viewer(system_id)
generate_maintenance_context(evidence)
```

Below hero visual:
Show pipeline strip:

```text
CAD / IFC / Synthetic Plant
→ 3D Tiles 1.1
→ Spatial Index
→ Knowledge Graph
→ MCP Tools
→ Agent + Viewer Actions
```

Tone:
Confident, precise, serious, enterprise-grade.

---

## Section 2 — The problem

Heading:

# Industrial 3D is not just triangles.

Copy:

Industrial models are heavy, messy, proprietary, and semantically rich. A pump is not just a mesh. A valve is not just geometry. A pipe segment belongs to a line, appears in a P&ID, connects to equipment, has isolation logic, and must map back to visual features in the viewer.

LLM agents cannot reason safely from triangles alone.

They need deterministic access to:

- stable object IDs
- engineering tags
- graph connectivity
- spatial proximity
- P&ID references
- datasheets
- work packages
- tile and feature mappings
- auditable tool results

Add a visual card titled:

`Why normal 3D viewers are not enough`

Compare:

Traditional 3D Viewer:

- renders geometry
- selects meshes
- shows properties
- disconnected from graph semantics
- no deterministic agent interface

TileGraphAgent:

- preserves industrial object identity
- maps visual features to engineering objects
- exports semantic graph relationships
- provides MCP tools for agents
- logs every action for auditability

---

## Section 3 — Core thesis

Heading:

# A shared identity layer across 3D, graph, and agents.

Copy:

TileGraphAgent is built around one principle: every visual feature, graph node, spatial index record, and MCP result must resolve to the same stable engineering object identity.

Use this identity stack:

```text
object_id
tag
source_id
revision_id
tile_id
feature_id
gltf_node
global_transform
aabb
```

Create a diagram:

```text
Engineering Object
   ├── glTF Node
   ├── 3D Tiles Feature
   ├── Spatial Index Record
   ├── Neo4j Graph Node
   ├── MCP Resource
   └── Viewer Selection
```

Supporting copy:

This is what allows an AI agent to answer engineering questions with evidence instead of hallucination.

---

## Section 4 — Platform architecture

Heading:

# End-to-end industrial 3D × AI pipeline.

Show architecture pipeline cards:

1. Ingest
   - Synthetic EPC plant model
   - IFC sample support
   - future RVM / NWD / NWC adapters

2. Normalize
   - industrial scene graph
   - stable object IDs
   - transforms
   - AABB bounding volumes

3. Generate geometry
   - procedural equipment
   - pipe segments
   - valves
   - supports
   - instance groups

4. Export GLB
   - glTF nodes
   - extras metadata
   - feature IDs
   - material groups

5. Build 3D Tiles
   - tileset.json
   - tile hierarchy
   - geometric error
   - object-to-feature mapping

6. Build spatial index
   - R-tree
   - AABB queries
   - nearby lookup
   - object-to-tile lookup

7. Export Knowledge Graph
   - Neo4j nodes
   - Cypher relationships
   - connectivity
   - document linkage

8. Expose MCP Bridge
   - deterministic tools
   - schema validation
   - audit logs
   - viewer commands

9. Operate viewer
   - CesiumJS
   - highlight
   - isolate
   - focus camera
   - issue creation

---

## Section 5 — Demo scenario

Heading:

# Ask engineering questions. Get visual, graph-backed answers.

Use this exact demo question:

> Find all pumps connected to LINE-1001, show their isolation valves, isolate the affected system in the viewer, and explain the maintenance impact.

Show the flow as a vertical timeline:

1. User asks the question in the agent panel.
2. Agent resolves `LINE-1001` using `search_object_by_tag`.
3. Agent queries connected pumps and valves.
4. Agent checks upstream/downstream relationships.
5. Agent maps graph objects to 3D Tiles feature IDs.
6. Agent sends viewer commands to isolate and highlight objects.
7. Viewer highlights pumps, valves, and pipe segments.
8. Agent returns evidence-backed maintenance context.
9. Audit log stores every tool call.

Add a mock agent response card:

```text
LINE-1001 is connected to pumps P-1001 and P-1002.

Isolation valves:
- XV-1001A upstream of P-1001
- XV-1001B downstream of P-1001
- XV-1002A upstream of P-1002

Viewer action:
- Isolated SYS-COOLING
- Highlighted 2 pumps, 4 valves, 11 pipe segments

Maintenance impact:
Shutting down LINE-1001 affects the cooling water loop in Area A.
Confidence: High for synthetic graph data. Real plant validation required before operational use.
```

Add audit log visual:

```json
{
  "session_id": "agent_session_001",
  "tool": "query_connected_components",
  "input": {
    "object_id": "obj_line_1001"
  },
  "output_count": 17,
  "status": "success",
  "timestamp": "2026-05-29T10:32:18Z"
}
```

---

## Section 6 — MCP Agent Bridge

Heading:

# Deterministic tools for engineering agents.

Copy:

TileGraphAgent does not allow the LLM to guess engineering facts or directly manipulate the model. The agent must use schema-bound MCP tools and return evidence from structured results.

Show MCP tools grid:

- `search_object_by_tag`
- `get_object_properties`
- `query_connected_components`
- `query_upstream_downstream`
- `query_objects_in_area`
- `query_nearby_objects`
- `get_tile_feature_mapping`
- `highlight_objects_in_viewer`
- `isolate_system_in_viewer`
- `focus_camera_on_objects`
- `create_issue_from_selection`
- `generate_maintenance_context`

Add safety rules card:

Agent rules:

- Never infer facts without tool evidence.
- Always resolve tags to object IDs.
- Always distinguish connectivity from proximity.
- Never trigger viewer actions without object mappings.
- Always include uncertainty.
- Always write audit-friendly answers.

---

## Section 7 — Knowledge Graph

Heading:

# Engineering semantics beyond the mesh.

Show Neo4j-style graph visualization with nodes:

- Plant
- Area
- System
- Line
- Pump
- Valve
- PipeSegment
- Instrument
- PID
- Datasheet
- WorkPackage
- Issue
- Tile
- Feature
- BoundingVolume

Show relationship examples:

```text
Pump PART_OF System
Valve CONNECTED_TO PipeSegment
Line APPEARS_IN_PID PID
Pump HAS_DATASHEET Datasheet
Feature MAPS_TO Object
Object HAS_BOUNDING_VOLUME AABB
Valve ISOLATED_BY Line
Issue AFFECTS Equipment
```

Add Cypher example card:

```cypher
MATCH (line:Line {tag: "LINE-1001"})-[:CONNECTED_TO*1..4]-(pump:Pump)
OPTIONAL MATCH (pump)-[:ISOLATED_BY]-(valve:Valve)
RETURN line.tag, pump.tag, collect(valve.tag) AS isolation_valves;
```

Copy:

The graph makes engineering relationships explicit, queryable, and inspectable by both humans and agents.

---

## Section 8 — 3D Tiles + glTF identity mapping

Heading:

# Every visible feature maps back to an engineering object.

Show a mapping card:

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

Show output layout:

```text
output/tiles/
  tileset.json
  content/
    area-a-root.glb
    area-a-piping.glb
    area-a-equipment.glb
  metadata/
    feature_table.json
    object_properties.json
    tile_feature_map.json
  index/
    spatial_index.json
  reports/
    validation_report.json
```

Copy:

TileGraphAgent uses 3D Tiles for scalable streaming, glTF/GLB for optimized geometry, and metadata mapping files to preserve the link between visual content and engineering semantics.

---

## Section 9 — CesiumJS Viewer

Heading:

# Agent-controlled 3D viewer actions.

Show viewer feature cards:

- Load generated `tileset.json`
- Select object
- Show property panel
- Search by tag
- Highlight object list
- Isolate system
- Focus camera
- Show bounding boxes
- Show connected components
- Open P&ID / datasheet links
- Create issue marker
- Show audit trail

Show WebSocket command example:

```json
{
  "type": "highlight_objects",
  "session_id": "agent_session_001",
  "objects": [
    {
      "object_id": "obj_pump_p_1001",
      "feature_id": 1201,
      "color": "#38BDF8"
    },
    {
      "object_id": "obj_valve_xv_1001a",
      "feature_id": 1407,
      "color": "#F59E0B"
    }
  ]
}
```

Copy:

The viewer is not just a display surface. It becomes the visual execution layer for deterministic agent actions.

---

## Section 10 — Technical stack

Heading:

# Built with production-relevant engineering tools.

Create stack grid:

## Rust pipeline

- `tilegraph-core`
- `tilegraph-synth`
- `tilegraph-ingest`
- `tilegraph-geometry`
- `tilegraph-gltf`
- `tilegraph-tiles`
- `tilegraph-spatial`
- `tilegraph-graph-export`
- `tilegraph-cli`

## TypeScript apps

- `tilegraphmcp`
- `tilegraph-viewer`

## Data systems

- Neo4j
- R-tree spatial index
- JSON metadata
- GLB / glTF
- 3D Tiles 1.1

## Viewer

- CesiumJS
- WebSocket command bridge
- REST property lookup
- Agent panel
- Audit trail panel

## Future production adapters

- RVM
- NWD / NWC
- IFC
- DWG metadata
- Smart3D / SP3D MDB-like tables
- P&ID linking
- revision comparison

---

## Section 11 — Validation and correctness

Heading:

# Built for correctness, not demo magic.

Copy:

Industrial agent systems must be verifiable. TileGraphAgent includes validation across geometry, graph, tiles, MCP schemas, and viewer mappings.

Show validation checklist:

- object ID uniqueness
- tag uniqueness
- graph relationship consistency
- missing geometry detection
- missing graph node detection
- missing tile mapping detection
- invalid bounding volume detection
- disconnected line segment detection
- orphan valve detection
- P&ID reference mismatch detection
- datasheet reference mismatch detection
- feature ID mismatch detection
- MCP output schema validation
- viewer selection mismatch detection

Show report card:

```json
{
  "objects_total": 168,
  "tags_unique": true,
  "missing_geometry": 0,
  "missing_graph_nodes": 0,
  "missing_tile_mappings": 0,
  "invalid_bounding_volumes": 0,
  "status": "pass"
}
```

---

## Section 12 — Portfolio / engineering credibility

Heading:

# Designed to demonstrate real industrial 3D platform skills.

Create mapping table:

| Capability                | TileGraphAgent evidence                               |
| ------------------------- | ----------------------------------------------------- |
| CAD/BIM pipeline thinking | Synthetic + IFC-ready ingestion architecture          |
| Rust systems design       | Modular Rust workspace and CLI                        |
| 3D Tiles knowledge        | tileset generation, bounding volumes, geometric error |
| glTF/GLB knowledge        | node mapping, extras metadata, feature IDs            |
| Spatial indexing          | R-tree object query and nearby lookup                 |
| Graph modeling            | Neo4j schema, Cypher queries, relationship types      |
| MCP agent bridge          | schema-bound tools, resources, audit logs             |
| CesiumJS viewer           | selection, highlight, isolate, focus camera           |
| Correctness discipline    | validation reports and manual verification            |
| AI-assisted workflow      | documented AI use with human verification             |

---

## Section 13 — CTA

Heading:

# Explore the architecture behind agentic industrial 3D.

CTA buttons:

Primary:
`Read the Technical Plan`

Secondary:
`View Demo Scenario`

Tertiary:
`Open GitHub Repository`

Final tagline:

TileGraphAgent bridges geometry, graph semantics, and AI agents — turning industrial 3D models into queryable, auditable engineering systems.

---

# 4. Page layout requirements

Create a single-page responsive landing page.

Desktop layout:

- max content width: 1200–1280px
- hero split layout
- alternating technical sections
- card-based architecture sections
- sticky top nav
- smooth scroll anchors

Mobile layout:

- stacked hero
- cards become single column
- diagrams become scrollable
- code blocks remain readable

Top navigation:

Left:

- TileGraphAgent logo wordmark

Center:

- Problem
- Architecture
- Demo
- MCP
- Viewer
- Stack

Right:

- GitHub button
- Demo button

Footer:

Include:

- TileGraphAgent
- Industrial CAD → 3D Tiles → Knowledge Graph → MCP Agent Bridge
- Built for industrial 3D, agentic CAD, digital twin, and engineering AI workflows.

---

# 5. Logo / brand direction

Create a simple technical brand mark:

Concept:

- Isometric tile / grid cell
- connected graph nodes
- subtle 3D cube outline
- cyan-blue technical glow
- small agent node bridge

Logo should feel like:

- industrial geometry
- graph intelligence
- 3D spatial data
- agent bridge

Do not make it look like:

- gaming logo
- crypto logo
- generic AI sparkle
- childish robot icon

---

# 6. Copy tone

Use language that is:

- precise
- technical
- confident
- concise
- enterprise-ready
- engineering-focused

Avoid vague phrases like:

- “revolutionary AI”
- “unlock the future”
- “seamless magic”
- “AI-powered everything”
- “next-gen solution” without explanation

Prefer concrete phrases:

- deterministic MCP tools
- stable object identity
- graph-backed reasoning
- feature-to-object mapping
- auditable viewer commands
- 3D Tiles metadata strategy
- spatial + semantic query layer

---

# 7. Visual components to include

Generate these visual components:

1. Hero 3D industrial viewer mockup
2. Agent tool-call panel
3. Pipeline flow strip
4. Identity mapping diagram
5. Architecture cards
6. Demo timeline
7. MCP tools grid
8. Knowledge Graph visualization
9. glTF metadata card
10. WebSocket viewer command card
11. Validation report card
12. Technical stack grid
13. Portfolio credibility table
14. Final CTA

---

# 8. Implementation target

Generate the landing page as a modern frontend implementation.

Preferred stack:

- React
- TypeScript
- Tailwind CSS
- Framer Motion
- lucide-react icons
- shadcn/ui style cards/buttons if available

Design details:

- use semantic HTML
- accessible color contrast
- responsive layout
- reusable components
- clean code structure
- no unnecessary dependencies
- no fake external links unless placeholders are clearly marked
- use realistic placeholder URLs:
  - `/docs/architecture`
  - `/demo`
  - `https://github.com/tilegraphagent/tilegraphagent.github.i`

Use animation carefully:

- fade-in sections
- subtle graph node pulse
- code cards slide in
- hero viewport slow glow
- no excessive motion

---

# 9. Required final output

Generate:

1. Complete landing page copy
2. Complete React + Tailwind implementation
3. Suggested color tokens
4. Suggested component structure
5. Responsive layout behavior
6. Optional future improvements

The final page should feel like a premium industrial AI infrastructure product, not a generic SaaS template.
