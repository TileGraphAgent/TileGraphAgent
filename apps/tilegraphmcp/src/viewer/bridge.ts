export type ViewerCommand =
  | { type: "highlight_objects"; object_ids: string[]; color?: string }
  | { type: "isolate_objects"; object_ids: string[] }
  | { type: "focus_camera"; object_ids: string[] }
  | { type: "show_bounding_boxes"; object_ids: string[] }
  | { type: "clear_highlights" }
  | { type: "create_issue_marker"; object_id: string; title: string; severity: string }
  | { type: "ping" }
  | { type: "pong" };

interface CommandEntry {
  id: number;
  timestamp: string;
  command: ViewerCommand;
}

const MAX_COMMANDS = 50;
// Consider viewer connected if it polled within this window
const CONNECTED_THRESHOLD_MS = 10_000;

export class HttpViewerBridge {
  private commands: CommandEntry[] = [];
  private nextId = 1;
  private lastPollAt: number | null = null;

  sendCommand(command: ViewerCommand): void {
    this.commands.push({ id: this.nextId++, timestamp: new Date().toISOString(), command });
    if (this.commands.length > MAX_COMMANDS) {
      this.commands.shift();
    }
    console.error(`[ViewerBridge] Queued command: ${command.type} (queue: ${this.commands.length})`);
  }

  getCommandsAfter(cursor: number | null): { commands: CommandEntry[]; next_cursor: number | null } {
    this.lastPollAt = Date.now();
    const filtered = cursor !== null ? this.commands.filter((c) => c.id > cursor) : [...this.commands];
    const last = filtered.at(-1);
    return {
      commands: filtered,
      next_cursor: last?.id ?? cursor,
    };
  }

  get connectedClients(): number {
    if (this.lastPollAt === null) return 0;
    return Date.now() - this.lastPollAt < CONNECTED_THRESHOLD_MS ? 1 : 0;
  }
}
