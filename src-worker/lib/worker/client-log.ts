import { apiFetch } from "../loopback-api-client";
import { CLIENT_LOG_THROTTLE_MS } from "./worker-defaults";

const CLIENT_LOG_STATE_MAX = 256;

const clientLogState = new Map<string, { at: number; muted: boolean }>();

function trimClientLogState(): void {
  if (clientLogState.size <= CLIENT_LOG_STATE_MAX) {
    return;
  }
  const entries = [...clientLogState.entries()].sort((a, b) => a[1].at - b[1].at);
  const removeCount = clientLogState.size - CLIENT_LOG_STATE_MAX;
  for (let index = 0; index < removeCount; index += 1) {
    const entry = entries[index];
    if (!entry) continue;
    clientLogState.delete(entry[0]);
  }
}

async function sendClientLogPayload(payload: Record<string, unknown>): Promise<void> {
  const body = JSON.stringify(payload);
  try {
    const response = await apiFetch("/api/logs/client-event", {
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
    // best-effort client log
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
    "websocket connected",
    "websocket closed",
    "websocket error",
    "watchdog forced rearm",
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
  trimClientLogState();
  void sendClientLogPayload(payload);
}

export function appendWorkerLog(message: string): void {
  if (shouldPersistWorkerLog(message)) {
    postClientLog(message);
  }
}
