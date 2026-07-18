import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { handleWsEvent } from "./stores/app";
import type { RuntimeStateSnapshot, WsMessage } from "./types";

export type RuntimeEventHandler = (message: WsMessage) => void;

/** Which snapshot fields to replay after connecting the live channel. */
export type SnapshotReplayMode = "dashboard" | "tts";

export type StartRuntimeEventChannelOptions = {
  snapshotMode?: SnapshotReplayMode;
};

let activeRuntimeEventUnlisten: UnlistenFn | null = null;

const RUNTIME_EVENT_LISTEN_MAX_ATTEMPTS = 3;
const RUNTIME_EVENT_LISTEN_RETRY_MS = 150;

function sleepMs(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

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

export function snapshotToRuntimeMessages(
  snapshot: RuntimeStateSnapshot,
  mode: SnapshotReplayMode = "dashboard",
): WsMessage[] {
  const messages: WsMessage[] = [];
  if (snapshot.runtime && typeof snapshot.runtime === "object") {
    messages.push({
      type: "runtime_update",
      payload: snapshot.runtime as Record<string, unknown>,
    });
  }

  if (mode === "tts") {
    // TTS only acts on runtime + twitch connection; skip heavy overlay/translation bodies.
    if (snapshot.twitch_connection && typeof snapshot.twitch_connection === "object") {
      messages.push({
        type: "twitch_connection_update",
        payload: snapshot.twitch_connection as Record<string, unknown>,
      });
    }
    return messages;
  }

  // Prefer overlay (live shape). Fall back to raw subtitle only when overlay is absent.
  if (snapshot.overlay && typeof snapshot.overlay === "object") {
    messages.push({
      type: "overlay_update",
      payload: snapshot.overlay as Record<string, unknown>,
    });
  } else if (snapshot.subtitle && typeof snapshot.subtitle === "object") {
    messages.push({
      type: "subtitle_payload_update",
      payload: snapshot.subtitle as Record<string, unknown>,
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
  // Replay Twitch connection status so the TTS window restores its connection UI after a
  // bus lag resync (review MED#7). Chat messages are ephemeral and not replayed.
  if (snapshot.twitch_connection && typeof snapshot.twitch_connection === "object") {
    messages.push({
      type: "twitch_connection_update",
      payload: snapshot.twitch_connection as Record<string, unknown>,
    });
  }
  return messages;
}

/**
 * Apply snapshot first, then drain live events that arrived during the snapshot await.
 * Ordering avoids both: (1) lost events before listen, (2) stale snapshot overwriting
 * a newer live frame that arrived while invoke was in flight.
 */
export function applySnapshotThenBufferedLive(
  handler: RuntimeEventHandler,
  snapshotMessages: WsMessage[],
  bufferedLive: WsMessage[],
): void {
  for (const message of snapshotMessages) {
    handler(message);
  }
  for (const message of bufferedLive) {
    handler(message);
  }
}

async function fetchSnapshotMessages(mode: SnapshotReplayMode): Promise<WsMessage[]> {
  try {
    const snapshot = await invoke<RuntimeStateSnapshot>("get_runtime_state_snapshot");
    return snapshotToRuntimeMessages(snapshot, mode);
  } catch {
    // Snapshot is best-effort; live channel still works without it.
    return [];
  }
}

export async function startRuntimeEventChannelWithHandler(
  handler: RuntimeEventHandler,
  onConnected?: () => void,
  options?: StartRuntimeEventChannelOptions,
): Promise<UnlistenFn | null> {
  const snapshotMode = options?.snapshotMode ?? "dashboard";
  for (let attempt = 1; attempt <= RUNTIME_EVENT_LISTEN_MAX_ATTEMPTS; attempt++) {
    try {
      const bufferedLive: WsMessage[] = [];
      let buffering = true;
      // Attach listen first and buffer until snapshot apply completes.
      const unlisten = await listen<unknown>("runtime-event", (event) => {
        const message = normalizeRuntimeEventMessage(event.payload);
        if (!message) {
          return;
        }
        if (buffering) {
          bufferedLive.push(message);
          return;
        }
        handler(message);
      });
      const previous = activeRuntimeEventUnlisten;
      activeRuntimeEventUnlisten = unlisten;
      previous?.();
      const snapshotMessages = await fetchSnapshotMessages(snapshotMode);
      applySnapshotThenBufferedLive(handler, snapshotMessages, bufferedLive);
      buffering = false;
      onConnected?.();
      return unlisten;
    } catch {
      if (attempt < RUNTIME_EVENT_LISTEN_MAX_ATTEMPTS) {
        await sleepMs(RUNTIME_EVENT_LISTEN_RETRY_MS * attempt);
      }
    }
  }
  return null;
}

export async function startRuntimeEventChannel(
  onConnected?: () => void,
): Promise<UnlistenFn | null> {
  return startRuntimeEventChannelWithHandler(handleWsEvent, onConnected, {
    snapshotMode: "dashboard",
  });
}
