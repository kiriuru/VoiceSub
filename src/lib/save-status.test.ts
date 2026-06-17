import { describe, expect, it } from "vitest";

import { formatSaveStatusDisplay, saveSnackbarDismissMs } from "./save-status";

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

  it("uses snackbar timing aligned with transient feedback", () => {
    expect(saveSnackbarDismissMs("success")).toBeGreaterThan(3000);
    expect(saveSnackbarDismissMs("error")).toBeGreaterThan(saveSnackbarDismissMs("success"));
  });
});
