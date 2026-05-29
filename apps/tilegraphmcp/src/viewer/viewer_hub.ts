import { DurableObject } from "cloudflare:workers";

export type ViewerCommand =
  | { type: "highlight_objects"; object_ids: string[]; color?: string }
  | { type: "isolate_objects"; object_ids: string[] }
  | { type: "focus_camera"; object_ids: string[] }
  | { type: "show_bounding_boxes"; object_ids: string[] }
  | { type: "clear_highlights" }
  | { type: "create_issue_marker"; object_id: string; title: string; severity: string }
  | { type: "ping" }
  | { type: "pong" };

// Durable Object that holds WebSocket sessions for all connected viewer tabs
export class ViewerHub extends DurableObject {
  private sessions: Set<WebSocket> = new Set();

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);

    if (url.pathname === "/ws") {
      // Upgrade to WebSocket
      const upgradeHeader = request.headers.get("Upgrade");
      if (upgradeHeader !== "websocket") {
        return new Response("Expected WebSocket upgrade", { status: 426 });
      }
      const pair = new WebSocketPair();
      const [client, server] = Object.values(pair);
      this.ctx.acceptWebSocket(server);
      this.sessions.add(server);
      server.addEventListener("close", () => this.sessions.delete(server));
      server.addEventListener("error", () => this.sessions.delete(server));
      return new Response(null, { status: 101, webSocket: client });
    }

    if (url.pathname === "/send" && request.method === "POST") {
      const command: ViewerCommand = await request.json();
      const msg = JSON.stringify(command);
      let sent = 0;
      for (const ws of this.sessions) {
        try {
          ws.send(msg);
          sent++;
        } catch {
          this.sessions.delete(ws);
        }
      }
      return Response.json({ sent, total: this.sessions.size });
    }

    if (url.pathname === "/status") {
      return Response.json({ connected_clients: this.sessions.size });
    }

    return new Response("Not found", { status: 404 });
  }
}

// Client used by tool handlers to send commands via the Durable Object
export class DurableViewerBridge {
  private stub: DurableObjectStub;

  constructor(namespace: DurableObjectNamespace) {
    // Single hub instance shared across all workers
    const id = namespace.idFromName("viewer-hub");
    this.stub = namespace.get(id);
  }

  async sendCommand(command: ViewerCommand): Promise<void> {
    await this.stub.fetch("http://do/send", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(command),
    });
  }

  async getStatus(): Promise<{ connected_clients: number }> {
    const resp = await this.stub.fetch("http://do/status");
    return resp.json();
  }

  // Compatibility shim used by tool handlers that call sendCommand
  getCommandHistory(): Array<{ timestamp: string; command: ViewerCommand }> {
    return [];
  }

  get connectedClients(): number {
    return 0;
  }
}
