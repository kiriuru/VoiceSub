import { describe, expect, it } from "vitest";

import { extractPrimaryFontFamily, replacePrimaryFontFamily } from "./font-catalog";

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

describe("replacePrimaryFontFamily", () => {
  it("preserves Cyrillic-capable fallbacks when swapping the primary face", () => {
    const chain =
      '"Mochiy Pop One Regular", "Comfortaa Bold", "Underdog Regular", "Comic Relief Bold", "Segoe UI", sans-serif';
    expect(replacePrimaryFontFamily(chain, '"Bangers Regular"')).toBe(
      '"Bangers Regular", "Comfortaa Bold", "Underdog Regular", "Comic Relief Bold", "Segoe UI", sans-serif',
    );
  });

  it("dedupes when the new primary already appears later in the stack", () => {
    const chain = '"Oswald Bold", "Montserrat Bold", "Impact", sans-serif';
    expect(replacePrimaryFontFamily(chain, '"Montserrat Bold"')).toBe(
      '"Montserrat Bold", "Impact", sans-serif',
    );
  });
});
