import { WebSocketServer, WebSocket } from "ws";

export type ViewerCommand =
  | { type: "highlight_objects"; object_ids: string[]; color?: string }
  | { type: "isolate_objects"; object_ids: string[] }
  | { type: "focus_camera"; object_ids: string[] }
  | { type: "show_bounding_boxes"; object_ids: string[] }
  | { type: "clear_highlights" }
  | { type: "create_issue_marker"; object_id: string; title: string; severity: string }
  | { type: "ping" }
  | { type: "pong" };

interface ViewerClient {
  id: string;
  ws: WebSocket;
  connectedAt: Date;
  lastPongAt: Date;
  isPrimary: boolean;
}

const COMMAND_QUEUE_SIZE = 10;
const HEARTBEAT_INTERVAL_MS = 30_000;
const PONG_TIMEOUT_MS = 5_000;

export class ViewerBridge {
  private wss: WebSocketServer | null = null;
  private clients: Map<string, ViewerClient> = new Map();
  private commandQueue: Array<{ timestamp: string; command: ViewerCommand }> = [];
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private port: number;

  constructor(port: number) {
    this.port = port;
  }

  async start(): Promise<void> {
    this.wss = new WebSocketServer({ port: this.port });

    this.wss.on("connection", (ws) => {
      const clientId = `viewer_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`;

      const isPrimary = this.clients.size === 0;
      if (isPrimary) {
        for (const c of this.clients.values()) c.isPrimary = false;
      }

      const client: ViewerClient = {
        id: clientId,
        ws,
        connectedAt: new Date(),
        lastPongAt: new Date(),
        isPrimary,
      };
      this.clients.set(clientId, client);
      console.error(`[ViewerBridge] ${clientId} connected (${this.clients.size} total, primary=${isPrimary})`);

      for (const { command } of this.commandQueue) {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify(command));
        }
      }

      ws.on("message", (data) => {
        try {
          const msg = JSON.parse(data.toString());
          if (msg.type === "pong") {
            client.lastPongAt = new Date();
          }
        } catch {
          /* ignore malformed */
        }
      });

      ws.on("close", () => {
        this.clients.delete(clientId);
        console.error(`[ViewerBridge] ${clientId} disconnected (${this.clients.size} remaining)`);
        const remaining = Array.from(this.clients.values());
        if (remaining.length > 0 && !remaining.some((c) => c.isPrimary)) {
          remaining[remaining.length - 1].isPrimary = true;
        }
      });

      ws.on("error", (err) => {
        console.error(`[ViewerBridge] ${clientId} error:`, err.message);
        this.clients.delete(clientId);
      });
    });

    this.heartbeatTimer = setInterval(() => {
      const now = Date.now();
      for (const [id, client] of this.clients) {
        if (client.ws.readyState !== WebSocket.OPEN) {
          this.clients.delete(id);
          continue;
        }
        const timeSincePong = now - client.lastPongAt.getTime();
        if (timeSincePong > HEARTBEAT_INTERVAL_MS + PONG_TIMEOUT_MS) {
          console.error(`[ViewerBridge] ${id} pong timeout — terminating`);
          client.ws.terminate();
          this.clients.delete(id);
          continue;
        }
        client.ws.send(JSON.stringify({ type: "ping" }));
      }
    }, HEARTBEAT_INTERVAL_MS);

    console.error(`[ViewerBridge] Listening on ws://localhost:${this.port}`);
  }

  sendCommand(command: ViewerCommand): void {
    this.commandQueue.push({ timestamp: new Date().toISOString(), command });
    if (this.commandQueue.length > COMMAND_QUEUE_SIZE) {
      this.commandQueue.shift();
    }

    const msg = JSON.stringify(command);
    let sent = 0;
    for (const client of this.clients.values()) {
      if (client.ws.readyState === WebSocket.OPEN) {
        client.ws.send(msg);
        sent++;
      }
    }
    console.error(`[ViewerBridge] Sent ${command.type} to ${sent}/${this.clients.size} clients`);
  }

  getCommandHistory() {
    return this.commandQueue;
  }

  get connectedClients(): number {
    return this.clients.size;
  }

  get primaryClientId(): string | undefined {
    return Array.from(this.clients.values()).find((c) => c.isPrimary)?.id;
  }

  async stop(): Promise<void> {
    if (this.heartbeatTimer) clearInterval(this.heartbeatTimer);
    this.wss?.close();
  }
}
