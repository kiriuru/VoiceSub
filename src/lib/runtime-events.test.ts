import { describe, expect, it } from "vitest";

import {
  applySnapshotThenBufferedLive,
  normalizeRuntimeEventMessage,
  snapshotToRuntimeMessages,
} from "./runtime-events";
import type { RuntimeStateSnapshot, WsMessage } from "./types";

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
  it("maps dashboard snapshot fields preferring overlay over subtitle", () => {
    const snapshot: RuntimeStateSnapshot = {
      rev: 3,
      runtime: { running: true, phase: "running" },
      subtitle: { sequence: 1, raw: true },
      overlay: { sequence: 1 },
      translation: { sequence: 1 },
      diagnostics: { browser_worker: { connected: true } },
    };

    const messages = snapshotToRuntimeMessages(snapshot, "dashboard");
    expect(messages.map((message) => message.type)).toEqual([
      "runtime_update",
      "overlay_update",
      "translation_update",
      "diagnostics_update",
    ]);
    expect(messages[0]?.payload).toEqual({ running: true, phase: "running" });
  });

  it("falls back to subtitle when overlay is missing", () => {
    const messages = snapshotToRuntimeMessages(
      {
        rev: 1,
        runtime: { running: true },
        subtitle: { sequence: 2 },
      },
      "dashboard",
    );
    expect(messages.map((message) => message.type)).toEqual([
      "runtime_update",
      "subtitle_payload_update",
    ]);
  });

  it("skips empty snapshot sections", () => {
    const messages = snapshotToRuntimeMessages({
      rev: 0,
      runtime: {},
    });
    expect(messages).toEqual([{ type: "runtime_update", payload: {} }]);
  });

  it("replays twitch connection status for the TTS window", () => {
    const messages = snapshotToRuntimeMessages(
      {
        rev: 5,
        runtime: { running: false },
        twitch_connection: { state: "connected", channel: "demo" },
      },
      "dashboard",
    );
    expect(messages.map((message) => message.type)).toEqual([
      "runtime_update",
      "twitch_connection_update",
    ]);
    expect(messages[1]?.payload).toEqual({ state: "connected", channel: "demo" });
  });

  it("tts mode only replays runtime and twitch connection", () => {
    const messages = snapshotToRuntimeMessages(
      {
        rev: 6,
        runtime: { running: true },
        subtitle: { sequence: 1 },
        overlay: { sequence: 1 },
        translation: { sequence: 1 },
        diagnostics: { ok: true },
        twitch_connection: { state: "connected" },
      },
      "tts",
    );
    expect(messages.map((message) => message.type)).toEqual([
      "runtime_update",
      "twitch_connection_update",
    ]);
  });
});

describe("applySnapshotThenBufferedLive", () => {
  it("applies snapshot first then drains live events so live wins over stale snapshot", () => {
    const applied: WsMessage[] = [];
    const handler = (message: WsMessage) => {
      applied.push(message);
    };
    applySnapshotThenBufferedLive(
      handler,
      [{ type: "overlay_update", payload: { sequence: 9 } }],
      [{ type: "overlay_update", payload: { sequence: 10 } }],
    );
    expect(applied.map((m) => m.payload.sequence)).toEqual([9, 10]);
  });
});
