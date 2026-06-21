import { describe, expect, it } from "vitest";

import { normalizeRuntimeEventMessage, snapshotToRuntimeMessages } from "./runtime-events";
import type { RuntimeStateSnapshot } from "./types";

describe("normalizeRuntimeEventMessage", () => {
  it("unwraps tauri runtime-event payload into ws message shape", () => {
    const message = normalizeRuntimeEventMessage({
      type: "subtitle_payload_update",
      payload: { sequence: 3, active_partial_text: "live" },
    });
    expect(message).toEqual({
      type: "subtitle_payload_update",
      payload: { sequence: 3, active_partial_text: "live" },
    });
  });
});

describe("snapshotToRuntimeMessages", () => {
  it("maps snapshot fields to ws-shaped runtime messages", () => {
    const snapshot: RuntimeStateSnapshot = {
      rev: 3,
      runtime: { running: true, phase: "running" },
      subtitle: { sequence: 1 },
      overlay: { sequence: 1 },
      translation: { sequence: 1 },
      diagnostics: { browser_worker: { connected: true } },
    };

    const messages = snapshotToRuntimeMessages(snapshot);
    expect(messages.map((message) => message.type)).toEqual([
      "runtime_update",
      "subtitle_payload_update",
      "overlay_update",
      "translation_update",
      "diagnostics_update",
    ]);
    expect(messages[0]?.payload).toEqual({ running: true, phase: "running" });
  });

  it("skips empty snapshot sections", () => {
    const messages = snapshotToRuntimeMessages({
      rev: 0,
      runtime: {},
    });
    expect(messages).toEqual([{ type: "runtime_update", payload: {} }]);
  });

  it("replays twitch connection status for the TTS window", () => {
    const messages = snapshotToRuntimeMessages({
      rev: 5,
      runtime: { running: false },
      twitch_connection: { state: "connected", channel: "demo" },
    });
    expect(messages.map((message) => message.type)).toEqual([
      "runtime_update",
      "twitch_connection_update",
    ]);
    expect(messages[1]?.payload).toEqual({ state: "connected", channel: "demo" });
  });
});
