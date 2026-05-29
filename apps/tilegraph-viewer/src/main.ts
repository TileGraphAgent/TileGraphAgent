import { initCesiumViewer } from "./viewer/cesium_init.js";
import { ViewerCommandClient } from "./agent/ws_client.js";
import { store } from "./state/store.js";
import { fetchAndRenderProperties } from "./ui/properties_panel.js";
import { initModelTree } from "./ui/model_tree.js";

const TILESET_PATH = import.meta.env.VITE_TILESET_PATH ?? "../../output/tiles/tileset.json";
const WS_URL = import.meta.env.VITE_WS_URL ?? "ws://localhost:9001";

async function main(): Promise<void> {
  const container = document.getElementById("cesium-container");
  if (!container) throw new Error("Missing #cesium-container");

  const tileGraph = await initCesiumViewer(
    "cesium-container",
    TILESET_PATH,
    async (objectId, tag) => {
      store.update({ selectedObjectId: objectId, selectedTag: tag });
      const panel = document.getElementById("selection-panel")!;
      await fetchAndRenderProperties(objectId, panel);
    }
  );

  const wsClient = new ViewerCommandClient(WS_URL, tileGraph);
  wsClient.connect();

  store.subscribe(renderAuditPanel);

  // Initialize model tree (non-blocking — shows error if MCP server not running)
  initModelTree(
    document.getElementById("model-tree-panel")!,
    (objectIds) => {
      tileGraph.isolateObjects(objectIds);
      store.update({ isolatedObjectIds: new Set(objectIds) });
    },
    (objectIds) => {
      tileGraph.highlightObjects(objectIds);
      store.update({ highlightedObjectIds: new Set(objectIds) });
    }
  );

  console.log("[TileGraphAgent Viewer] ready");
}

function renderAuditPanel(state: ReturnType<typeof store.get>): void {
  const panel = document.getElementById("audit-panel");
  if (!panel) return;
  const last5 = state.auditLog.slice(-5).reverse();
  panel.innerHTML = `
    <h3>Audit Log</h3>
    ${last5.map((e) => `<div class="audit-entry"><span class="tool">${e.tool_name}</span> ${e.timestamp}</div>`).join("")}
  `;
}

main().catch(console.error);
