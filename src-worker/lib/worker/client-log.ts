import { CLIENT_LOG_THROTTLE_MS } from "./worker-defaults";

const clientLogState = new Map<string, { at: number; muted: boolean }>();

async function sendClientLogPayload(payload: Record<string, unknown>): Promise<void> {
  const body = JSON.stringify(payload);
  try {
    const response = await fetch("/api/logs/client-event", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body,
    });
    const result = await response.json().catch(() => null);
    if (result && result.logged === false) {
      const key = `${payload.channel}:${payload.source}:${payload.message}`;
      clientLogState.set(key, { at: Date.now(), muted: true });
    }
  } catch {
    if (typeof navigator?.sendBeacon === "function") {
      try {
        navigator.sendBeacon("/api/logs/client-event", new Blob([body], { type: "application/json" }));
      } catch {
        // best-effort
      }
    }
  }
}

function shouldPersistWorkerLog(message: string): boolean {
  const normalized = String(message || "")
    .trim()
    .toLowerCase();
  if (!normalized) {
    return false;
  }
  return [
    "worker initialized",
    "worker ready",
    "settings loaded",
    "settings load failed",
    "document visibility changed",
    "window blur",
    "window focus",
    "requesting microphone permission",
    "microphone permission granted",
    "microphone permission failed",
    "recognition.start failed",
    "recognition.onerror",
    "websocket connected",
    "websocket closed",
    "websocket error",
    "watchdog forced rearm",
    "restart cancelled",
    "auto-restart stopped",
    "stop requested by user",
  ].some((token) => normalized.includes(token));
}

export function postClientLog(message: string, details?: Record<string, unknown>): void {
  const payload: Record<string, unknown> = {
    channel: "browser_worker",
    source: "browser-worker",
    message: String(message || "").trim(),
  };
  if (!payload.message) {
    return;
  }
  if (details && typeof details === "object") {
    payload.details = details;
  }
  const key = `${payload.channel}:${payload.source}:${payload.message}`;
  const last = clientLogState.get(key);
  if (last && last.muted && Date.now() - last.at < CLIENT_LOG_THROTTLE_MS) {
    return;
  }
  if (last && Date.now() - last.at < CLIENT_LOG_THROTTLE_MS) {
    return;
  }
  clientLogState.set(key, { at: Date.now(), muted: false });
  void sendClientLogPayload(payload);
}

export function appendWorkerLog(message: string): void {
  if (shouldPersistWorkerLog(message)) {
    postClientLog(message);
  }
}
