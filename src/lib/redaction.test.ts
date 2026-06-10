import { describe, expect, it } from "vitest";

import { REDACTED_VALUE, redactObject, redactText } from "./redaction";

describe("redactObject", () => {
  it("redacts nested sensitive keys and endpoint query values", () => {
    const payload = {
      translation: {
        api_key: "secret-value",
        endpoint: "https://example.test/translate?token=abc123&mode=fast",
      },
      providers: { backup: { key: "legacy-secret" } },
      remote: {
        pair_code: "123456",
        controller: {
          worker_url: "http://192.168.1.10:8765",
        },
      },
    };

    const redacted = redactObject(payload);
    expect(redacted.translation.api_key).toBe(REDACTED_VALUE);
    expect(redacted.providers.backup.key).toBe(REDACTED_VALUE);
    expect(redacted.translation.endpoint).toMatch(/token=(\[redacted\]|%5Bredacted%5D)/i);
    expect(redacted.translation.endpoint).toContain("mode=fast");
    expect(redacted.remote.pair_code).toBe(REDACTED_VALUE);
    expect(redacted.remote.controller.worker_url).toBe("http://192.168.1.10:8765");
  });
});

describe("redactText", () => {
  it("redacts bearer tokens in text", () => {
    expect(redactText("Authorization failed for Bearer super-secret-token")).toBe(
      "Authorization failed for Bearer [redacted]",
    );
  });

  it("redacts key query parameter in text", () => {
    expect(redactText("key=legacy-secret&pair=sst-123")).toContain(`key=${REDACTED_VALUE}`);
  });
});
