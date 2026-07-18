import { describe, expect, it } from "vitest";

import { isValidProfileName, normalizeProfileName } from "./profile-name";

describe("profile-name", () => {
  it("accepts simple names", () => {
    expect(isValidProfileName("stream")).toBe(true);
    expect(isValidProfileName("default")).toBe(true);
    expect(isValidProfileName("  night  ")).toBe(true);
    expect(normalizeProfileName("  night  ")).toBe("night");
  });

  it("rejects path traversal and separators", () => {
    expect(isValidProfileName("..")).toBe(false);
    expect(isValidProfileName("../x")).toBe(false);
    expect(isValidProfileName("a/b")).toBe(false);
    expect(isValidProfileName("a\\b")).toBe(false);
  });

  it("rejects Windows reserved and illegal characters", () => {
    expect(isValidProfileName("CON")).toBe(false);
    expect(isValidProfileName("nul")).toBe(false);
    expect(isValidProfileName("com1")).toBe(false);
    expect(isValidProfileName("bad:name")).toBe(false);
    expect(isValidProfileName("a*b")).toBe(false);
    expect(isValidProfileName("ends.")).toBe(false);
  });
});
