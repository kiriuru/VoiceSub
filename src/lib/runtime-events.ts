import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { handleWsEvent } from "./stores/app";
import type { RuntimeStateSnapshot, WsMessage } from "./types";

export type RuntimeEventHandler = (message: WsMessage) => void;

export function normalizeRuntimeEventMessage(raw: unknown): WsMessage | null {
  if (!raw || typeof raw !== "object") {
    return null;
  }
  const record = raw as Record<string, unknown>;
  const type = String(record.type || "").trim();
  if (!type) {
    return null;
  }
  const payload =
    record.payload && typeof record.payload === "object"
      ? (record.payload as Record<string, unknown>)
      : {};
  return { type, payload };
}

export function snapshotToRuntimeMessages(snapshot: RuntimeStateSnapshot): WsMessage[] {
  const messages: WsMessage[] = [];
  if (snapshot.runtime && typeof snapshot.runtime === "object") {
    messages.push({
      type: "runtime_update",
      payload: snapshot.runtime as Record<string, unknown>,
    });
  }
  if (snapshot.subtitle && typeof snapshot.subtitle === "object") {
    messages.push({
      type: "subtitle_payload_update",
      payload: snapshot.subtitle as Record<string, unknown>,
    });
  }
  if (snapshot.overlay && typeof snapshot.overlay === "object") {
    messages.push({
      type: "overlay_update",
      payload: snapshot.overlay as Record<string, unknown>,
    });
  }
  if (snapshot.translation && typeof snapshot.translation === "object") {
    messages.push({
      type: "translation_update",
      payload: snapshot.translation as Record<string, unknown>,
    });
  }
  if (snapshot.diagnostics && typeof snapshot.diagnostics === "object") {
    messages.push({
      type: "diagnostics_update",
      payload: snapshot.diagnostics as Record<string, unknown>,
    });
  }
  return messages;
}

async function applyRuntimeSnapshot(handler: RuntimeEventHandler): Promise<void> {
  try {
    const snapshot = await invoke<RuntimeStateSnapshot>("get_runtime_state_snapshot");
    for (const message of snapshotToRuntimeMessages(snapshot)) {
      handler(message);
    }
  } catch {
    // Snapshot is best-effort; live channel still works without it.
  }
}

export async function startRuntimeEventChannelWithHandler(
  handler: RuntimeEventHandler,
  onConnected?: () => void,
): Promise<UnlistenFn | null> {
  try {
    await applyRuntimeSnapshot(handler);
    const unlisten = await listen<unknown>("runtime-event", (event) => {
      const message = normalizeRuntimeEventMessage(event.payload);
      if (message) {
        handler(message);
      }
    });
    onConnected?.();
    return unlisten;
  } catch {
    return null;
  }
}

export async function startRuntimeEventChannel(
  onConnected?: () => void,
): Promise<UnlistenFn | null> {
  return startRuntimeEventChannelWithHandler(handleWsEvent, onConnected);
}
