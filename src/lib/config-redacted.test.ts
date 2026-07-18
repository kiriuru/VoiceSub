import { describe, expect, it } from "vitest";

import { containsRedactedPlaceholders } from "./config-redacted";
import { REDACTED_VALUE } from "./redaction";

describe("containsRedactedPlaceholders", () => {
  it("detects nested redacted secrets", () => {
    expect(
      containsRedactedPlaceholders({
        translation: { provider_settings: { deepl: { api_key: REDACTED_VALUE } } },
      }),
    ).toBe(true);
  });

  it("ignores clean configs", () => {
    expect(
      containsRedactedPlaceholders({
        translation: { provider_settings: { deepl: { api_key: "real-key" } } },
      }),
    ).toBe(false);
  });
});
