import type { WsConnectionStatus, WsMessage } from "./types";
import { ttsTrace } from "./tts-trace";

export class EventsSocket {
  private socket: WebSocket | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private backoffMs = 1000;
  private manualClose = false;
  private connectionId = 0;

  constructor(
    private onMessage: (msg: WsMessage) => void,
    private onStatus: (status: WsConnectionStatus) => void,
  ) {}

  connect() {
    if (
      this.socket?.readyState === WebSocket.OPEN ||
      this.socket?.readyState === WebSocket.CONNECTING
    ) {
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
      ttsTrace("ws", "connected", { url });
      this.socket?.send("ping");
    });

    this.socket.addEventListener("message", (event) => {
      if (connectionId !== this.connectionId) return;
      this.handleMessage(String(event.data));
    });

    this.socket.addEventListener("close", () => {
      if (connectionId !== this.connectionId) return;
      this.onStatus("disconnected");
      ttsTrace("ws", "disconnected", { manual: this.manualClose });
      this.socket = null;
      if (!this.manualClose) this.scheduleReconnect();
    });

    this.socket.addEventListener("error", () => {
      if (connectionId !== this.connectionId) return;
      ttsTrace("ws", "error", {});
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
    ttsTrace("ws", "reconnect_scheduled", { backoff_ms: this.backoffMs });
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, this.backoffMs);
    this.backoffMs = Math.min(10000, this.backoffMs * 2);
  }

  private handleMessage(raw: string) {
    try {
      const message = JSON.parse(raw) as WsMessage;
      const type = String(message.type || "").trim();
      if (type === "hello" || type === "connected" || type === "pong") return;
      if (
        type === "subtitle_payload_update" ||
        type === "overlay_update" ||
        type === "runtime_update" ||
        type === "runtime_status" ||
        type === "twitch_chat_message" ||
        type === "twitch_connection_update"
      ) {
        const payload = (message.payload || {}) as Record<string, unknown>;
        if (type === "twitch_chat_message") {
          ttsTrace("ws", "twitch_chat", {
            user: payload.user ?? "",
            speakable: payload.speakable ?? true,
            lang: payload.language ?? "",
          });
        } else if (type === "twitch_connection_update") {
          ttsTrace("ws", "twitch_connection", {
            state: payload.state ?? "",
            channel: payload.channel ?? "",
          });
        } else {
          ttsTrace("ws", "message", {
            type,
            sequence: payload.sequence ?? null,
            lifecycle: payload.lifecycle_state ?? null,
          });
        }
      }
      this.onMessage({ ...message, type });
    } catch {
      // ignore non-json frames
    }
  }
}
