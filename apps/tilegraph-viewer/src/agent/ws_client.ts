import type { TileGraphViewer } from "../viewer/cesium_init.js";
import { store } from "../state/store.js";

type ViewerCommand =
  | { type: "highlight_objects"; object_ids: string[]; color?: string }
  | { type: "isolate_objects"; object_ids: string[] }
  | { type: "focus_camera"; object_ids: string[] }
  | { type: "show_bounding_boxes"; object_ids: string[] }
  | { type: "clear_highlights" }
  | { type: "create_issue_marker"; object_id: string; title: string; severity: string };

export class ViewerCommandClient {
  private ws: WebSocket | null = null;
  private url: string;
  private viewer: TileGraphViewer;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(url: string, viewer: TileGraphViewer) {
    this.url = url;
    this.viewer = viewer;
  }

  connect(): void {
    try {
      this.ws = new WebSocket(this.url);
      this.ws.onopen = () => {
        console.log("[WS] Connected to MCP server bridge");
        store.update({ auditLog: [...store.get().auditLog] }); // trigger re-render
      };
      this.ws.onmessage = (event) => this.handleMessage(event.data as string);
      this.ws.onclose = () => {
        console.log("[WS] Disconnected — reconnecting in 3s");
        this.scheduleReconnect();
      };
      this.ws.onerror = (err) => {
        console.error("[WS] Error", err);
      };
    } catch (err) {
      console.error("[WS] Failed to connect:", err);
      this.scheduleReconnect();
    }
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer) return;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, 3000);
  }

  private handleMessage(raw: string): void {
    let cmd: ViewerCommand;
    try {
      cmd = JSON.parse(raw) as ViewerCommand;
    } catch {
      console.error("[WS] Invalid message:", raw);
      return;
    }

    console.log("[WS] Received:", cmd.type);
    const state = store.get();

    switch (cmd.type) {
      case "highlight_objects":
        this.viewer.highlightObjects(cmd.object_ids);
        store.update({
          highlightedObjectIds: new Set(cmd.object_ids),
        });
        break;

      case "isolate_objects":
        this.viewer.isolateObjects(cmd.object_ids);
        store.update({
          isolatedObjectIds: new Set(cmd.object_ids),
        });
        break;

      case "focus_camera":
        this.viewer.focusCameraOn(cmd.object_ids);
        break;

      case "show_bounding_boxes":
        this.viewer.showBoundingBoxes(true);
        break;

      case "clear_highlights":
        this.viewer.clearHighlights();
        store.update({
          highlightedObjectIds: new Set(),
          isolatedObjectIds: null,
        });
        break;

      case "create_issue_marker":
        console.log(`[Issue] ${cmd.severity}: ${cmd.title} on ${cmd.object_id}`);
        break;
    }
  }
}
