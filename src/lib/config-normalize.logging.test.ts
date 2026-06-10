import { describe, expect, it } from "vitest";

import { normalizeConfigPayload } from "./config-normalize";

describe("logging config normalization", () => {
  it("defaults full logging to false", () => {
    const normalized = normalizeConfigPayload({});
    expect(normalized.logging?.full_enabled).toBe(false);
  });

  it("preserves explicit true", () => {
    const normalized = normalizeConfigPayload({
      logging: { full_enabled: true },
    });
    expect(normalized.logging?.full_enabled).toBe(true);
  });
});
