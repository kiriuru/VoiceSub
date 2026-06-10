import { describe, expect, it } from "vitest";

import {
  createWsStaleGuardState,
  isWsEventStale,
  normalizeWsEventType,
} from "./ws-stale-guard";

describe("isWsEventStale", () => {
  it("accepts newer timestamp after sequence reset", () => {
    const guard = createWsStaleGuardState();
    isWsEventStale(guard, "overlay_update", {
      created_at_ms: 1000,
      event_sequence: 99,
    });
    const stale = isWsEventStale(guard, "overlay_update", {
      created_at_ms: 2000,
      event_sequence: 1,
    });
    expect(stale).toBe(false);
  });

  it("rejects older timestamp", () => {
    const guard = createWsStaleGuardState();
    isWsEventStale(guard, "overlay_update", {
      created_at_ms: 2000,
      event_sequence: 5,
    });
    const stale = isWsEventStale(guard, "overlay_update", {
      created_at_ms: 1000,
      event_sequence: 9,
    });
    expect(stale).toBe(true);
  });

  it("tracks normalized runtime_update under runtime_status key", () => {
    const guard = createWsStaleGuardState();
    const eventType = normalizeWsEventType("runtime_update");
    expect(eventType).toBe("runtime_status");
    isWsEventStale(guard, eventType, {
      created_at_ms: 1000,
      event_sequence: 1,
    });
    const stale = isWsEventStale(guard, eventType, {
      created_at_ms: 500,
      event_sequence: 2,
    });
    expect(stale).toBe(true);
  });
});
