import type { AsrManagerHost } from "./types";

const attachedSockets = new WeakSet<WebSocket>();

export function buildAsrWorkerWebSocketUrl(): string {
  const protocol = typeof location !== "undefined" && location.protocol === "https:" ? "wss" : "ws";
  const host = typeof location !== "undefined" ? location.host : "127.0.0.1";
  return `${protocol}://${host}/ws/asr_worker`;
}

export function parseBrowserAsrControlMessage(raw: string): { type: string; action: string } | null {
  let message: Record<string, unknown> | null = null;
  try {
    message = JSON.parse(raw) as Record<string, unknown>;
  } catch {
    return null;
  }
  if (!message || typeof message !== "object") {
    return null;
  }
  const type = String(message.type || "")
    .trim()
    .toLowerCase();
  if (type !== "browser_asr_control") {
    return null;
  }
  return {
    type,
    action: String(message.action || "")
      .trim()
      .toLowerCase(),
  };
}

export function attachSocketListeners(manager: AsrManagerHost, socket: WebSocket): void {
  if (!socket || attachedSockets.has(socket)) {
    return;
  }
  attachedSockets.add(socket);

  socket.addEventListener("open", () => {
    if (manager.state.socket !== socket) {
      return;
    }
    manager.state.websocketReady = true;
    manager.state.socketDegraded = false;
    manager.refreshDegradedReasonInternal();
    manager.appendLogInternal("websocket connected");
    manager.updateCountersInternal();
    manager.emitWorkerStatus("socket-open");
    manager.emitHeartbeat("socket-open");
    if (
      manager.state.desiredRunning &&
      manager.state.browserSupervisorState !== "running" &&
      manager.state.browserSupervisorState !== "starting"
    ) {
      manager.scheduleRestartInternal("websocket_reconnect");
    }
  });

  socket.addEventListener("close", () => {
    if (manager.state.socket !== socket) {
      return;
    }
    manager.state.websocketReady = false;
    manager.state.socketDegraded = Boolean(manager.state.desiredRunning);
    manager.refreshDegradedReasonInternal();
    manager.appendLogInternal("websocket closed");
    manager.updateCountersInternal();
    manager.state.socket = null;
    if (manager.state.desiredRunning) {
      manager.setStatusInternal("socket-reconnecting");
      manager.state.reconnectTimer = window.setTimeout(
        () => manager.ensureSocketConnectedInternal(),
        manager.restartDelayByReasonMs.websocket_reconnect
      );
    }
  });

  socket.addEventListener("error", () => {
    if (manager.state.socket !== socket) {
      return;
    }
    manager.state.websocketReady = false;
    manager.state.socketDegraded = Boolean(manager.state.desiredRunning);
    manager.refreshDegradedReasonInternal();
    manager.appendLogInternal("websocket error");
    manager.updateCountersInternal();
  });

  socket.addEventListener("message", (event) => {
    if (manager.state.socket !== socket) {
      return;
    }
    manager.handleSocketMessageInternal(String(event.data));
  });
}

export function ensureSocketConnected(manager: AsrManagerHost): void {
  const socket = manager.state.socket;
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
    return;
  }
  manager.clearReconnectTimerInternal();
  const nextSocket = new WebSocket(buildAsrWorkerWebSocketUrl());
  manager.state.socket = nextSocket;
  attachSocketListeners(manager, nextSocket);
}
