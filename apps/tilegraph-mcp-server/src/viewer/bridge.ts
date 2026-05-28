import { WebSocketServer, WebSocket } from "ws";

export type ViewerCommand =
  | { type: "highlight_objects"; object_ids: string[]; color?: string }
  | { type: "isolate_objects"; object_ids: string[] }
  | { type: "focus_camera"; object_ids: string[] }
  | { type: "show_bounding_boxes"; object_ids: string[] }
  | { type: "clear_highlights" }
  | { type: "create_issue_marker"; object_id: string; title: string; severity: string };

export class ViewerBridge {
  private wss: WebSocketServer | null = null;
  private clients: Set<WebSocket> = new Set();
  private port: number;
  private commandHistory: Array<{ timestamp: string; command: ViewerCommand }> = [];

  constructor(port: number) {
    this.port = port;
  }

  async start(): Promise<void> {
    this.wss = new WebSocketServer({ port: this.port });
    this.wss.on("connection", (ws) => {
      this.clients.add(ws);
      console.error(`[ViewerBridge] Viewer connected (${this.clients.size} total)`);
      ws.on("close", () => {
        this.clients.delete(ws);
        console.error(`[ViewerBridge] Viewer disconnected (${this.clients.size} remaining)`);
      });
      ws.on("error", (err) => {
        console.error(`[ViewerBridge] WebSocket error:`, err);
      });
    });
    console.error(`[ViewerBridge] Listening on ws://localhost:${this.port}`);
  }

  sendCommand(command: ViewerCommand): void {
    const msg = JSON.stringify(command);
    this.commandHistory.push({
      timestamp: new Date().toISOString(),
      command,
    });
    let sent = 0;
    for (const client of this.clients) {
      if (client.readyState === WebSocket.OPEN) {
        client.send(msg);
        sent++;
      }
    }
    console.error(
      `[ViewerBridge] Sent ${command.type} to ${sent}/${this.clients.size} clients`
    );
  }

  getCommandHistory() {
    return this.commandHistory;
  }

  get connectedClients(): number {
    return this.clients.size;
  }
}
