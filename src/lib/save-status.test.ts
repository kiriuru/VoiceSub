import { describe, expect, it } from "vitest";

import { formatSaveStatusDisplay } from "./save-status";

describe("formatSaveStatusDisplay", () => {
  it("recomputes success message when locale changes", () => {
    const state = {
      tone: "success" as const,
      liveApplied: true,
      restartReasonKeys: [],
    };
    expect(formatSaveStatusDisplay(state, null, "en")).toContain("immediately");
    expect(formatSaveStatusDisplay(state, null, "ru")).not.toContain("immediately");
  });
});
