import { describe, expect, it } from "vitest";

import { normalizeOverlayPayload } from "./overlay-normalizer";

describe("normalizeOverlayPayload", () => {
  it("preserves known lifecycle states", () => {
    for (const state of [
      "idle",
      "partial_only",
      "completed_only",
      "completed_with_partial",
    ]) {
      const out = normalizeOverlayPayload({ lifecycle_state: state });
      expect(out.lifecycle_state).toBe(state);
    }
  });

  it("coerces unknown lifecycle_state to idle", () => {
    expect(normalizeOverlayPayload({ lifecycle_state: "bogus" }).lifecycle_state).toBe(
      "idle",
    );
    expect(normalizeOverlayPayload({ lifecycle_state: "" }).lifecycle_state).toBe("idle");
  });

  it("normalizes visible_items shape", () => {
    const out = normalizeOverlayPayload({
      visible_items: [
        {
          kind: "source",
          text: "hello",
          slot_id: "source",
          target_lang: "en",
        },
      ],
    });
    expect(out.visible_items).toHaveLength(1);
    expect(out.visible_items[0]).toMatchObject({
      kind: "source",
      text: "hello",
      slot_id: "source",
      target_lang: "en",
      style_slot: "",
    });
  });

  it("defaults show_source and show_translations to true", () => {
    const out = normalizeOverlayPayload({});
    expect(out.show_source).toBe(true);
    expect(out.show_translations).toBe(true);
  });

  it("clamps invalid preset to stacked", () => {
    expect(normalizeOverlayPayload({ preset: "bogus" }).preset).toBe("stacked");
    expect(normalizeOverlayPayload({ preset: "single" }).preset).toBe("single");
  });
});
