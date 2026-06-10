import type { WsMessage } from "./types";
import {
  createWsStaleGuardState,
  isWsEventStale,
  normalizeWsEventType,
} from "./ws-stale-guard";

export type WsStatus = "connecting" | "connected" | "disconnected";

export class EventsSocket {
  private socket: WebSocket | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private backoffMs = 1000;
  private manualClose = false;
  private connectionId = 0;
  private staleGuard = createWsStaleGuardState();

  constructor(
    private onMessage: (msg: WsMessage) => void,
    private onStatus: (status: WsStatus) => void,
  ) {}

  connect() {
    if (this.socket?.readyState === WebSocket.OPEN || this.socket?.readyState === WebSocket.CONNECTING) {
      return;
    }
    const url = `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws/events`;
    this.manualClose = false;
    this.onStatus("connecting");
    const connectionId = ++this.connectionId;
    this.socket = new WebSocket(url);

    this.socket.addEventListener("open", () => {
      if (connectionId !== this.connectionId) return;
      this.backoffMs = 1000;
      this.onStatus("connected");
      this.socket?.send("ping");
    });

    this.socket.addEventListener("message", (event) => {
      if (connectionId !== this.connectionId) return;
      this.handleMessage(String(event.data));
    });

    this.socket.addEventListener("close", () => {
      if (connectionId !== this.connectionId) return;
      this.onStatus("disconnected");
      this.socket = null;
      if (!this.manualClose) this.scheduleReconnect();
    });

    this.socket.addEventListener("error", () => {
      if (connectionId !== this.connectionId) return;
      this.socket?.close();
    });
  }

  disconnect() {
    this.manualClose = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.socket?.close();
    this.socket = null;
    this.onStatus("disconnected");
  }

  private scheduleReconnect() {
    if (this.reconnectTimer) return;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, this.backoffMs);
    this.backoffMs = Math.min(10000, this.backoffMs * 2);
  }

  private handleMessage(raw: string) {
    try {
      const message = JSON.parse(raw) as WsMessage;
      const type = normalizeWsEventType(String(message.type || "").trim());
      if (type === "hello" || type === "connected") return;
      const payload = message.payload as Record<string, unknown> | undefined;
      if (isWsEventStale(this.staleGuard, type, payload)) return;
      this.onMessage({ ...message, type });
    } catch {
      // ignore non-json
    }
  }
}
