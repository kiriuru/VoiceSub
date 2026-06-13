import { describe, expect, it } from "vitest";
import { shouldHoldWakeLock } from "./tts-keepalive";

describe("shouldHoldWakeLock", () => {
  it("requires runtime, enabled, and busy engines", () => {
    expect(
      shouldHoldWakeLock({
        runtimeActive: true,
        ttsEnabled: true,
        enginesBusy: true,
      }),
    ).toBe(true);
  });

  it("does not hold wake lock when idle", () => {
    expect(
      shouldHoldWakeLock({
        runtimeActive: true,
        ttsEnabled: true,
        enginesBusy: false,
      }),
    ).toBe(false);
  });

  it("does not hold wake lock when runtime is stopped", () => {
    expect(
      shouldHoldWakeLock({
        runtimeActive: false,
        ttsEnabled: true,
        enginesBusy: true,
      }),
    ).toBe(false);
  });
});
