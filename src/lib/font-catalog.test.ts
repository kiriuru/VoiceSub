import { describe, expect, it } from "vitest";

import { extractPrimaryFontFamily } from "./font-catalog";

describe("extractPrimaryFontFamily", () => {
  it("returns the first quoted family from a CSS chain", () => {
    expect(
      extractPrimaryFontFamily(
        '"VT323 Regular", "PT Mono Regular", "Consolas", monospace',
      ),
    ).toBe('"VT323 Regular"');
  });

  it("falls back to the first bare token when no quotes are present", () => {
    expect(extractPrimaryFontFamily("Segoe UI, Tahoma, sans-serif")).toBe("Segoe UI");
  });

  it("returns empty string for blank input", () => {
    expect(extractPrimaryFontFamily("")).toBe("");
    expect(extractPrimaryFontFamily("   ")).toBe("");
  });
});
