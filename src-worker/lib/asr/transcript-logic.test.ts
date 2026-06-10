import { describe, expect, it } from "vitest";
import { createBrowserAsrStateSeed } from "./session-state";
import {
  ensureClientSegmentId,
  normalizeTranscriptText,
  shouldSuppressDuplicatePartial,
  shouldSuppressFinal,
} from "./transcript-logic";

describe("transcript-logic", () => {
  it("normalizes whitespace", () => {
    expect(normalizeTranscriptText("  hello   world  ")).toBe("hello world");
  });

  it("dedupes identical partials", () => {
    const state = createBrowserAsrStateSeed();
    state.currentSegmentLastPartialText = "hello";
    expect(shouldSuppressDuplicatePartial(state, "hello")).toBe(true);
    expect(state.duplicatePartialSuppressed).toBe(1);
  });

  it("assigns stable client segment ids", () => {
    const state = createBrowserAsrStateSeed({ sessionId: "test-session" });
    const first = ensureClientSegmentId(state);
    const second = ensureClientSegmentId(state);
    expect(first).toBe(second);
    expect(first).toContain("test-session");
  });

  it("suppresses duplicate finals", () => {
    const state = createBrowserAsrStateSeed();
    state.currentSegmentLastFinalText = "done";
    expect(shouldSuppressFinal(state, "done")).toBe(true);
    expect(state.duplicateFinalSuppressed).toBe(1);
  });
});
