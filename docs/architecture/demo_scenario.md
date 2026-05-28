# Final Demo Scenario

## Question
> "Find all pumps connected to LINE-1001, show their isolation valves, isolate the affected system in the viewer, and explain the maintenance impact."

## Full Tool Chain

### Step 1 — Resolve LINE-1001

**Tool:** `search_object_by_tag`
```json
{ "tag": "LINE-1001" }
```
**Result:**
```json
{
  "found": true,
  "tag": "LINE-1001",
  "object_id": "obj_a3f2...",
  "class": "Line",
  "status": "Active",
  "tile_id": "area-a/content"
}
```

### Step 2 — Find connected pumps

**Tool:** `query_connected_components`
```json
{ "object_id": "obj_a3f2..." }
```
**Neo4j Cypher (executed internally):**
```cypher
MATCH (p:Pump)-[:CONNECTED_TO]->(l:Line {tag: "LINE-1001"})
RETURN p.object_id, p.tag, p.name, p.status, p.tile_id, p.feature_id
```
**Result:**
```json
{
  "connected_objects": [
    { "object_id": "obj_pump_...", "tag": "P-1001", "class": "Pump", "rel_type": "CONNECTED_TO" }
  ]
}
```

### Step 3 — Find upstream/downstream objects

**Tool:** `query_upstream_downstream`
```json
{ "object_id": "obj_pump_...", "direction": "both", "max_hops": 3 }
```

### Step 4 — Get tile feature mappings

**Tool:** `get_tile_feature_mapping`
```json
{ "object_ids": ["obj_pump_...", "obj_valve_1001A", "obj_valve_1001B"] }
```
**Result:**
```json
{
  "found_count": 3,
  "mappings": [
    { "object_id": "obj_pump_...", "feature_id": 1201, "tile_id": "area-a/content", "found": true },
    { "object_id": "obj_valve_1001A", "feature_id": 44, "found": true },
    { "object_id": "obj_valve_1001B", "feature_id": 45, "found": true }
  ]
}
```

### Step 5 — Isolate system in viewer

**Tool:** `isolate_system_in_viewer`
```json
{ "object_ids": ["obj_pump_...", "obj_valve_1001A", "obj_valve_1001B", "obj_a3f2..."] }
```
**WebSocket message sent to viewer:**
```json
{ "type": "isolate_objects", "object_ids": ["obj_pump_...", "..."] }
```

### Step 6 — Highlight pumps and valves

**Tool:** `highlight_objects_in_viewer`
```json
{ "object_ids": ["obj_pump_...", "obj_valve_1001A", "obj_valve_1001B"], "color": "agent_highlight" }
```

### Step 7 — Generate maintenance context

**Tool:** `generate_maintenance_context`
```json
{ "line_tag": "LINE-1001" }
```
**Neo4j Cypher (executed):**
```cypher
MATCH (l:Line {tag: "LINE-1001"})
OPTIONAL MATCH (pump:Pump)-[:CONNECTED_TO]->(l)
OPTIONAL MATCH (valve:Valve)-[:PART_OF|ISOLATED_BY]->(l)
OPTIONAL MATCH (instr:Instrument)-[:PART_OF]->(l)
OPTIONAL MATCH (seg:PipeSegment)-[:PART_OF]->(l)
RETURN l.tag, l.object_id,
       collect(DISTINCT pump.tag) AS connected_pumps,
       collect(DISTINCT valve.tag) AS isolation_valves,
       collect(DISTINCT instr.tag) AS instruments,
       count(DISTINCT seg) AS segment_count
```
**Result:**
```json
{
  "line_tag": "LINE-1001",
  "connected_pumps": ["P-1001"],
  "isolation_valves": ["V-10001A", "V-10001B", "V-10001C", "V-10001D"],
  "instruments": ["FT-1001", "PT-1001"],
  "segment_count": 16,
  "maintenance_steps": [
    "Isolate line by closing valves: V-10001A, V-10001B, V-10001C, V-10001D",
    "Stop connected pumps: P-1001",
    "Verify instrument readings are safe: FT-1001, PT-1001",
    "Depressurize 16 pipe segments",
    "Verify zero energy state before work begins"
  ]
}
```

## Final Agent Response

```
## Answer

LINE-1001 is a Cooling Water line in Area A (SYS-PLT-COOLING).

1 pump is connected: **P-1001** (Cooling Pump, 22kW, ACTIVE)
4 isolation valves identified: V-10001A, V-10001B, V-10001C, V-10001D
2 instruments on line: FT-1001 (flow transmitter), PT-1001 (pressure transmitter)

The affected system has been isolated in the viewer (16 pipe segments + pump + valves highlighted).

## Maintenance Impact

To take LINE-1001 out of service:
1. Close isolation valves: V-10001A, V-10001B, V-10001C, V-10001D
2. Stop pump P-1001
3. Verify FT-1001 flow = 0, PT-1001 pressure = safe
4. Depressurize 16 pipe segments
5. Verify zero energy state

## Evidence

| Fact | Source | Confidence |
|------|--------|------------|
| LINE-1001 exists | search_object_by_tag | HIGH |
| P-1001 connected via CONNECTED_TO | query_connected_components | HIGH |
| Valves isolated via ISOLATED_BY/PART_OF | generate_maintenance_context | HIGH |
| Viewer isolation | isolate_system_in_viewer | HIGH |

## Safety Note

SYNTHETIC DATA: Verify against physical P&ID and site LOTO procedure before any work.
```

## Audit Log Sample

```jsonl
{"session_id":"session_1234","timestamp":"2026-05-29T10:00:01Z","tool_name":"search_object_by_tag","input":{"tag":"LINE-1001"},"output_summary":"found=true object_id=obj_a3f2...","duration_ms":8}
{"session_id":"session_1234","timestamp":"2026-05-29T10:00:02Z","tool_name":"query_connected_components","input":{"object_id":"obj_a3f2..."},"output_summary":"connected_count=5","duration_ms":12}
{"session_id":"session_1234","timestamp":"2026-05-29T10:00:03Z","tool_name":"generate_maintenance_context","input":{"line_tag":"LINE-1001"},"output_summary":"pumps=1 valves=4 instruments=2","duration_ms":15}
{"session_id":"session_1234","timestamp":"2026-05-29T10:00:04Z","tool_name":"isolate_system_in_viewer","input":{"object_ids":["obj_pump_..."]},"output_summary":"isolated_count=20","duration_ms":3}
```
