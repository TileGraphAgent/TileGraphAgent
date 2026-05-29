# Prompt 3 — CesiumJS Viewer: Core Features

## Your role

You are implementing production improvements to **TileGraphAgent**, an industrial 3D platform. This session covers **Project 5, Stages 5.1–5.4** from `plan.md`: making the CesiumJS viewer actually work — correct feature picking, per-object highlighting, an engineering properties panel that fetches from the MCP server, and a collapsible model tree panel.

**Prerequisites:**

- Project 1 (Prompt 1) must be complete — pipeline compiles and tests pass
- Project 2.1 (Prompt 2) should be complete — `EXT_structural_metadata` makes `getProperty("tag")` work correctly; if not complete yet, the feature picking will fall back to reading `node.extras`

## Repository overview

- **Root:** `/Users/thanh/Workspace/TileGraphAgent`
- **Viewer app:** `apps/tilegraphviewer/`
- **MCP server app:** `apps/tilegraphmcp/`
- **Viewer entry:** `apps/tilegraphviewer/src/main.ts`
- **Viewer HTML:** `apps/tilegraphviewer/index.html`
- **Cesium init:** `apps/tilegraphviewer/src/viewer/cesium_init.ts`
- **State store:** `apps/tilegraphviewer/src/state/store.ts`
- **WS client:** `apps/tilegraphviewer/src/agent/ws_client.ts`

**Install and dev commands:**

```bash
# From repo root
cd apps/tilegraphviewer && npm install && npm run dev
# Viewer runs at http://localhost:5173

cd apps/tilegraphmcp && npm install && npm run dev
# MCP server starts on stdio; REST endpoints added in this session on :9000
```

## What currently exists (read before editing)

Read the following files in full before making any changes:

- `apps/tilegraphviewer/src/viewer/cesium_init.ts` — current feature picking (broken), highlight stubs
- `apps/tilegraphviewer/src/main.ts` — app entry point
- `apps/tilegraphviewer/src/state/store.ts` — `ViewerState` type
- `apps/tilegraphviewer/index.html` — HTML structure with panels
- `apps/tilegraphmcp/src/db/neo4j.ts` — `getObjectProperties` method
- `apps/tilegraphmcp/src/index.ts` — MCP server startup

## Stage 5.1 — Correct feature picking

### Problem

The current pick handler in `cesium_init.ts` uses:

```typescript
const picked = viewer.scene.pick(movement.position);
if (picked instanceof Cesium.Cesium3DTileFeature) { ... }
```

This is unreliable because:

1. The feature ID attribute `_FEATURE_ID_0` must be read via the `EXT_mesh_features` extension, not plain picking
2. The `featureIdToObjectId` and `objectIdToFeatureId` maps are declared but never populated
3. `getProperty("object_id")` returns `undefined` until `EXT_structural_metadata` is active

### Fix in `apps/tilegraphviewer/src/viewer/cesium_init.ts`

**Replace** the existing pick handler and add tile-load population of the lookup maps:

```typescript
// At module top, replace the empty map declarations:
export const featureIdToObjectId: Map<number, string> = new Map()
export const objectIdToFeatureId: Map<string, number> = new Map()

// After tileset loads, wire the tileVisible event:
tileset.tileVisible.addEventListener((tile: Cesium.Cesium3DTile) => {
  const content = tile.content
  if (!content) return
  const featuresLength = content.featuresLength
  for (let i = 0; i < featuresLength; i++) {
    try {
      const feature = content.getFeature(i)
      const oid = feature.getProperty("object_id") as string | undefined
      const fid = feature.getProperty("feature_id") as number | undefined
      if (oid && fid != null) {
        featureIdToObjectId.set(fid, oid)
        objectIdToFeatureId.set(oid, fid)
      }
    } catch {
      // Some tiles may not support getFeature — skip silently
    }
  }
})

// Replace the LEFT_CLICK handler:
viewer.screenSpaceEventHandler.setInputAction((movement: Cesium.ScreenSpaceEventHandler.PositionedEvent) => {
  const picked = viewer.scene.pick(movement.position)
  if (!Cesium.defined(picked)) return

  if (picked instanceof Cesium.Cesium3DTileFeature) {
    // Primary path: EXT_structural_metadata properties
    const objectId = picked.getProperty("object_id") as string | undefined
    const tag = picked.getProperty("tag") as string | undefined
    if (objectId) {
      onObjectSelected(objectId, tag ?? null)
      return
    }

    // Fallback: read from node extras (pre-EXT_structural_metadata)
    // node.extras is not directly accessible via getProperty, so check the mesh name
    const meshName = (picked as any)._content?._model?._loader?.gltfJson?.nodes?.find(
      (n: any) => n.extras?.feature_id === picked.featureId,
    )?.extras
    if (meshName?.object_id) {
      onObjectSelected(meshName.object_id, meshName.tag ?? null)
    }
  }
}, Cesium.ScreenSpaceEventType.LEFT_CLICK)
```

**Also update the `TileGraphViewer` interface** to expose the lookup maps:

```typescript
export interface TileGraphViewer {
  viewer: Cesium.Viewer
  tilesetRef: { tileset: Cesium.Cesium3DTileset | null }
  featureIdToObjectId: Map<number, string>
  objectIdToFeatureId: Map<string, number>
  highlightObjects: (objectIds: string[], color?: string) => void
  clearHighlights: () => void
  isolateObjects: (objectIds: string[]) => void
  focusCameraOn: (objectIds: string[]) => void
  showBoundingBoxes: (show: boolean) => void
}
```

Return these maps from `initCesiumViewer`.

---

## Stage 5.2 — Per-object highlight using `Cesium3DTileStyle` conditions

### Problem

The current `highlightObjects` uses a generic style string that doesn't correctly match feature properties. The correct approach uses a `conditions` array in `Cesium3DTileStyle`.

### Fix in `apps/tilegraphviewer/src/viewer/cesium_init.ts`

Add color constants and a helper:

```typescript
const HIGHLIGHT_COLOR_HEX = "#00CCFF"
const ISSUE_COLOR_HEX = "#FF3333"
const NORMAL_COLOR_HEX = "#CCCCCC"
const DIM_COLOR_HEX = "#555555"

function buildHighlightStyle(highlightIds: string[], isolatedIds: string[] | null): Cesium.Cesium3DTileStyle {
  if (isolatedIds !== null && isolatedIds.length > 0) {
    // Isolation mode: only show selected objects
    const idList = isolatedIds.map((id) => `'${id}'`).join(",")
    return new Cesium.Cesium3DTileStyle({
      show: `Boolean([${idList}].indexOf(String(\${object_id})) >= 0)`,
      color: {
        conditions: [
          [`[${idList}].indexOf(String(\${object_id})) >= 0`, `color('${HIGHLIGHT_COLOR_HEX}', 1.0)`],
          ["true", `color('${NORMAL_COLOR_HEX}', 0.0)`],
        ],
      },
    })
  }

  if (highlightIds.length > 0) {
    // Highlight mode: show highlighted bright, others dim
    const idList = highlightIds.map((id) => `'${id}'`).join(",")
    return new Cesium.Cesium3DTileStyle({
      show: "true",
      color: {
        conditions: [
          [`[${idList}].indexOf(String(\${object_id})) >= 0`, `color('${HIGHLIGHT_COLOR_HEX}', 1.0)`],
          ["true", `color('${DIM_COLOR_HEX}', 0.5)`],
        ],
      },
    })
  }

  // Default style
  return new Cesium.Cesium3DTileStyle({
    color: `color('${NORMAL_COLOR_HEX}', 0.9)`,
  })
}
```

Replace the `highlightObjects`, `clearHighlights`, and `isolateObjects` implementations:

```typescript
const highlightObjects = (objectIds: string[], _color?: string): void => {
  if (!tilesetRef.tileset) return
  tilesetRef.tileset.style = buildHighlightStyle(objectIds, null)
}

const clearHighlights = (): void => {
  if (!tilesetRef.tileset) return
  tilesetRef.tileset.style = buildHighlightStyle([], null)
}

const isolateObjects = (objectIds: string[]): void => {
  if (!tilesetRef.tileset) return
  tilesetRef.tileset.style = buildHighlightStyle([], objectIds)
}

const focusCameraOn = (objectIds: string[]): void => {
  if (!tilesetRef.tileset) return
  // Compute combined bounding sphere from feature AABBs
  // Fallback: zoom to full tileset
  viewer.zoomTo(tilesetRef.tileset)
}

const showBoundingBoxes = (show: boolean): void => {
  if (tilesetRef.tileset) {
    tilesetRef.tileset.debugShowBoundingVolume = show
  }
}
```

---

## Stage 5.3 — Properties panel with MCP REST fetch

### Step A — Add a REST HTTP server to the MCP server

The viewer needs a REST API since it runs in a browser and cannot call MCP protocol directly.

**File: `apps/tilegraphmcp/src/index.ts`**

Add an HTTP server alongside the MCP stdio server. Install `express` and `@types/express`:

```bash
cd apps/tilegraphmcp
npm install express
npm install --save-dev @types/express
```

In `src/index.ts`, after starting the MCP server:

```typescript
import express from "express"

// Start REST API for viewer
const app = express()
app.use(express.json())

// CORS for local dev
app.use((req, res, next) => {
  res.header("Access-Control-Allow-Origin", "*")
  res.header("Access-Control-Allow-Headers", "Content-Type")
  next()
})

// GET /objects/:id — full properties for one object
app.get("/objects/:id", async (req, res) => {
  try {
    const results = await neo4j.getObjectProperties(req.params.id)
    if (results.length === 0) {
      return res.status(404).json({ error: "NOT_FOUND", object_id: req.params.id })
    }
    const node = results[0] as Record<string, unknown>
    const obj = (node.o ?? node) as Record<string, unknown>
    const props = (obj as any).properties ?? obj
    res.json({ found: true, object_id: req.params.id, properties: props })
  } catch (err) {
    res.status(503).json({ error: "GRAPH_UNAVAILABLE", message: String(err) })
  }
})

// GET /health
app.get("/health", async (_req, res) => {
  const check = await neo4j.healthCheck()
  res.json({
    status: check.connected ? "ok" : "degraded",
    neo4j: check,
    spatial_index_records: spatialIndex.count,
  })
})

const REST_PORT = parseInt(process.env.REST_PORT ?? "9000")
app.listen(REST_PORT, () => {
  console.error(`[REST API] listening on http://localhost:${REST_PORT}`)
})
```

Also add `healthCheck()` to `Neo4jClient` if it doesn't already exist (from Prompt 4 — add it here if not done):

```typescript
async healthCheck(): Promise<{ connected: boolean; latency_ms: number }> {
    const t0 = Date.now();
    try {
        await this.query("RETURN 1 AS ok");
        return { connected: true, latency_ms: Date.now() - t0 };
    } catch {
        return { connected: false, latency_ms: -1 };
    }
}
```

### Step B — Properties panel in the viewer

**New file: `apps/tilegraphviewer/src/ui/properties_panel.ts`**

```typescript
const MCP_REST_BASE = import.meta.env.VITE_MCP_REST_URL ?? "http://localhost:9000"

interface ObjectProperties {
  object_id: string
  tag?: string
  name?: string
  class?: string
  status?: string
  tile_id?: string
  feature_id?: number
  [key: string]: unknown
}

export async function fetchAndRenderProperties(objectId: string, panelEl: HTMLElement): Promise<void> {
  panelEl.innerHTML = `<h3>Properties</h3><p class="loading">Loading ${objectId.slice(0, 16)}...</p>`

  try {
    const res = await fetch(`${MCP_REST_BASE}/objects/${encodeURIComponent(objectId)}`)
    if (!res.ok) {
      panelEl.innerHTML = `<h3>Properties</h3><p class="error">Not found (${res.status})</p>`
      return
    }
    const data: { found: boolean; properties: ObjectProperties } = await res.json()
    panelEl.innerHTML = renderPropertiesTable(data.properties ?? {})
  } catch (err) {
    panelEl.innerHTML = `<h3>Properties</h3>
            <p class="error">MCP server unreachable.<br/>Start: <code>npm run dev</code> in apps/tilegraphmcp</p>`
  }
}

function renderPropertiesTable(props: Record<string, unknown>): string {
  const priority = [
    "tag",
    "name",
    "class",
    "status",
    "fluid",
    "design_pressure_bar",
    "design_temperature_c",
    "power_kw",
    "volume_m3",
    "nominal_bore_mm",
  ]
  const shown = new Set<string>()
  let rows = ""

  // Priority fields first
  for (const key of priority) {
    if (key in props && props[key] != null) {
      rows += `<tr><td class="prop-key">${key}</td><td class="prop-val">${props[key]}</td></tr>`
      shown.add(key)
    }
  }
  // Remaining fields
  for (const [key, val] of Object.entries(props)) {
    if (!shown.has(key) && val != null && !key.startsWith("aabb_")) {
      const display = typeof val === "object" ? JSON.stringify(val) : String(val)
      rows += `<tr><td class="prop-key">${key}</td><td class="prop-val">${display}</td></tr>`
    }
  }

  return `<h3>Properties</h3>
        <table class="prop-table"><tbody>${rows}</tbody></table>`
}
```

**Update `apps/tilegraphviewer/src/main.ts`** to call `fetchAndRenderProperties` on selection:

```typescript
import { fetchAndRenderProperties } from "./ui/properties_panel.js"

// In the onObjectSelected callback:
const tileGraph = await initCesiumViewer("cesium-container", TILESET_PATH, async (objectId, tag) => {
  store.update({ selectedObjectId: objectId, selectedTag: tag })
  const panel = document.getElementById("selection-panel")!
  await fetchAndRenderProperties(objectId, panel)
})
```

**Update `apps/tilegraphviewer/index.html`** — add CSS for the properties table:

In the `<style>` block, add:

```css
.prop-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 11px;
}
.prop-table td {
  padding: 3px 6px;
  border-bottom: 1px solid #1a3050;
}
.prop-key {
  color: #78909c;
  width: 45%;
}
.prop-val {
  color: #e0e0e0;
  word-break: break-all;
}
.loading {
  color: #546e7a;
  font-size: 11px;
  font-style: italic;
}
.error {
  color: #ef5350;
  font-size: 11px;
}
```

---

## Stage 5.4 — Model tree panel

**New file: `apps/tilegraphviewer/src/ui/model_tree.ts`**

```typescript
const MCP_REST_BASE = import.meta.env.VITE_MCP_REST_URL ?? "http://localhost:9000"

interface TreeNode {
  id: string
  tag: string
  name: string
  class: string
  children?: TreeNode[]
  objectIds?: string[] // leaf objects belonging to this node
}

export async function initModelTree(
  panelEl: HTMLElement,
  onIsolate: (objectIds: string[]) => void,
  onSelect: (objectIds: string[]) => void,
): Promise<void> {
  panelEl.innerHTML = `<h3>Model Tree</h3><p class="loading">Loading hierarchy...</p>`
  try {
    const res = await fetch(`${MCP_REST_BASE}/hierarchy`)
    if (!res.ok) {
      panelEl.innerHTML = `<h3>Model Tree</h3><p class="error">Hierarchy unavailable</p>`
      return
    }
    const tree: TreeNode[] = await res.json()
    panelEl.innerHTML = `<h3>Model Tree</h3>` + renderTree(tree, onIsolate, onSelect)
    attachTreeHandlers(panelEl, onIsolate, onSelect)
  } catch {
    panelEl.innerHTML = `<h3>Model Tree</h3><p class="error">MCP server unreachable</p>`
  }
}

function renderTree(nodes: TreeNode[], onIsolate: Function, onSelect: Function): string {
  return nodes
    .map((node) => {
      const hasChildren = node.children && node.children.length > 0
      const icon = hasChildren ? "▶" : "•"
      const isolateBtn = node.objectIds?.length
        ? `<button class="tree-isolate" data-ids="${(node.objectIds ?? []).join(",")}">⊡</button>`
        : ""
      const childrenHtml = hasChildren
        ? `<div class="tree-children" style="display:none">${renderTree(node.children!, onIsolate, onSelect)}</div>`
        : ""
      return `
        <div class="tree-node" data-class="${node.class}">
            <div class="tree-row">
                <span class="tree-toggle ${hasChildren ? "has-children" : ""}">${icon}</span>
                <span class="tree-label" data-ids="${(node.objectIds ?? []).join(",")}"
                      title="${node.tag}">${node.tag || node.name}</span>
                ${isolateBtn}
            </div>
            ${childrenHtml}
        </div>`
    })
    .join("")
}

function attachTreeHandlers(
  panelEl: HTMLElement,
  onIsolate: (ids: string[]) => void,
  onSelect: (ids: string[]) => void,
): void {
  panelEl.querySelectorAll(".tree-toggle.has-children").forEach((el) => {
    el.addEventListener("click", (e) => {
      const row = (e.target as Element).closest(".tree-node")
      const children = row?.querySelector(".tree-children") as HTMLElement | null
      if (children) {
        const isOpen = children.style.display !== "none"
        children.style.display = isOpen ? "none" : "block"
        ;(e.target as Element).textContent = isOpen ? "▶" : "▼"
      }
    })
  })

  panelEl.querySelectorAll(".tree-label").forEach((el) => {
    el.addEventListener("click", () => {
      const ids = (el.getAttribute("data-ids") ?? "").split(",").filter(Boolean)
      if (ids.length > 0) onSelect(ids)
    })
  })

  panelEl.querySelectorAll(".tree-isolate").forEach((el) => {
    el.addEventListener("click", (e) => {
      e.stopPropagation()
      const ids = (el.getAttribute("data-ids") ?? "").split(",").filter(Boolean)
      if (ids.length > 0) onIsolate(ids)
    })
  })
}
```

**Add `/hierarchy` REST endpoint** to `apps/tilegraphmcp/src/index.ts`:

```typescript
app.get("/hierarchy", async (_req, res) => {
  try {
    // Query area → system → line structure
    const rows = await neo4j.query<{
      area_tag: string
      area_name: string
      area_id: string
      sys_tag: string
      sys_name: string
      sys_id: string
      line_tag: string
      line_id: string
    }>(`
            MATCH (a:Area)
            OPTIONAL MATCH (s:System)-[:PART_OF]->(a)
            OPTIONAL MATCH (l:Line)-[:PART_OF]->(s)
            RETURN a.tag AS area_tag, a.name AS area_name, a.object_id AS area_id,
                   s.tag AS sys_tag, s.name AS sys_name, s.object_id AS sys_id,
                   l.tag AS line_tag, l.object_id AS line_id
            ORDER BY a.tag, s.tag, l.tag
        `)

    // Group into tree
    const areaMap = new Map<string, any>()
    for (const row of rows) {
      if (!areaMap.has(row.area_id)) {
        areaMap.set(row.area_id, {
          id: row.area_id,
          tag: row.area_tag,
          name: row.area_name,
          class: "Area",
          children: new Map(),
          objectIds: [],
        })
      }
      const area = areaMap.get(row.area_id)!
      if (row.sys_id && !area.children.has(row.sys_id)) {
        area.children.set(row.sys_id, {
          id: row.sys_id,
          tag: row.sys_tag,
          name: row.sys_name,
          class: "System",
          children: new Map(),
          objectIds: [],
        })
      }
      if (row.line_id && row.sys_id) {
        const sys = area.children.get(row.sys_id)!
        if (!sys.children.has(row.line_id)) {
          sys.children.set(row.line_id, {
            id: row.line_id,
            tag: row.line_tag,
            name: row.line_tag,
            class: "Line",
            children: new Map(),
            objectIds: [row.line_id],
          })
        }
      }
    }

    // Flatten Maps to arrays for JSON
    function flatten(node: any): any {
      return {
        ...node,
        children: Array.from(node.children.values()).map(flatten),
      }
    }

    res.json(Array.from(areaMap.values()).map(flatten))
  } catch (err) {
    res.status(503).json({ error: "GRAPH_UNAVAILABLE", message: String(err) })
  }
})
```

**Update `apps/tilegraphviewer/index.html`** — add a model tree panel and CSS:

Replace the `<div id="sidebar">` content to add a tree panel:

```html
<div id="sidebar">
  <div class="panel" id="model-tree-panel">
    <h3>Model Tree</h3>
    <p style="color:#546e7a; font-size:11px">Loading...</p>
  </div>

  <div class="panel" id="selection-panel">
    <h3>Properties</h3>
    <p style="color:#546e7a; font-size:11px">Click an object.</p>
  </div>

  <div class="panel" id="audit-panel">
    <h3>Audit Log</h3>
    <p style="color:#546e7a; font-size:11px">No tool calls yet.</p>
  </div>

  <!-- agent panel unchanged below ... -->
</div>
```

Add CSS for tree:

```css
.tree-node {
  font-size: 11px;
}
.tree-row {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 0;
  cursor: pointer;
}
.tree-row:hover {
  background: #0f3460;
}
.tree-toggle {
  color: #546e7a;
  width: 12px;
  flex-shrink: 0;
}
.tree-toggle.has-children {
  cursor: pointer;
  color: #4fc3f7;
}
.tree-label {
  flex: 1;
  color: #b0bec5;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tree-children {
  padding-left: 14px;
}
.tree-isolate {
  background: none;
  border: 1px solid #1565c0;
  color: #4fc3f7;
  font-size: 10px;
  padding: 1px 4px;
  cursor: pointer;
  border-radius: 3px;
  flex-shrink: 0;
}
.tree-isolate:hover {
  background: #1565c0;
}
```

**Update `apps/tilegraphviewer/src/main.ts`** to initialize the tree:

```typescript
import { initModelTree } from "./ui/model_tree.js"

async function main(): Promise<void> {
  // ... existing code ...

  // Initialize model tree
  initModelTree(
    document.getElementById("model-tree-panel")!,
    (objectIds) => {
      tileGraph.isolateObjects(objectIds)
      store.update({ isolatedObjectIds: new Set(objectIds) })
    },
    (objectIds) => {
      tileGraph.highlightObjects(objectIds)
      store.update({ highlightedObjectIds: new Set(objectIds) })
    },
  )
}
```

---

## Verification sequence

```bash
# 1. Start Neo4j (required for property fetch and hierarchy)
docker-compose up -d neo4j

# 2. Import graph data (if not already done)
cargo run --bin tilegraph -- build-graph
cat output/graph/schema.cypher output/graph/import.cypher | \
    docker exec -i tilegraph-agent-neo4j-1 cypher-shell -u neo4j -p password

# 3. Start MCP server REST API
cd apps/tilegraphmcp && npm install && npm run dev &

# 4. Start viewer
cd apps/tilegraphviewer && npm install && npm run dev &

# 5. Verify REST API works
curl http://localhost:9000/health
# Should return: {"status":"ok","neo4j":{"connected":true,...}}

# 6. Test property fetch
curl "http://localhost:9000/objects/$(cargo run --bin tilegraph -- inspect-object P-10101 2>/dev/null | grep object_id | awk '{print $3}')"
# Should return JSON with class, tag, design_pressure_bar, etc.

# 7. Test hierarchy endpoint
curl http://localhost:9000/hierarchy | python3 -m json.tool | head -30
# Should show Area → System → Line tree

# 8. Open viewer in browser
open http://localhost:5173
# Manual checks:
# - Model tree shows Area A, Area B with child systems and lines
# - Clicking a Line node highlights objects and selects them
# - Clicking the ⊡ button isolates just those objects
# - Clicking on a 3D object in the viewport fetches and shows properties
# - Properties panel shows tag, class, fluid, design_pressure_bar
```

**Done when:**

- `http://localhost:9000/health` returns `status: ok`
- `http://localhost:9000/hierarchy` returns the tree JSON
- Clicking a pump in the 3D viewer shows its properties in the panel
- Clicking a line in the model tree highlights the related objects
- The `⊡` isolate button hides all other objects
- Viewer console shows no uncaught errors
