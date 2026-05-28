import { initCesiumViewer } from "./viewer/cesium_init.js";
import { ViewerCommandClient } from "./agent/ws_client.js";
import { store } from "./state/store.js";

const TILESET_PATH = import.meta.env.VITE_TILESET_PATH ?? "../../output/tiles/tileset.json";
const WS_URL = import.meta.env.VITE_WS_URL ?? "ws://localhost:9001";

async function main(): Promise<void> {
  // DOM structure is in index.html
  const container = document.getElementById("cesium-container");
  if (!container) throw new Error("Missing #cesium-container");

  const tileGraph = await initCesiumViewer(
    "cesium-container",
    TILESET_PATH,
    (objectId, tag) => {
      store.update({ selectedObjectId: objectId, selectedTag: tag });
      renderSelectionPanel(objectId, tag);
    }
  );

  const wsClient = new ViewerCommandClient(WS_URL, tileGraph);
  wsClient.connect();

  // Subscribe to state changes
  store.subscribe(renderAuditPanel);

  console.log("[TileGraphAgent Viewer] ready");
}

function renderSelectionPanel(objectId: string, tag: string | null): void {
  const panel = document.getElementById("selection-panel");
  if (!panel) return;
  panel.innerHTML = `
    <h3>Selected Object</h3>
    <p><b>ID:</b> ${objectId}</p>
    <p><b>Tag:</b> ${tag ?? "(no tag)"}</p>
  `;
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
