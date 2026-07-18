import { describe, expect, it } from "vitest";

import {
  buildSaveStatusMessage,
  getRestartRequiredReasons,
} from "./config-restart";
import type { ConfigPayload } from "./types";

const baseConfig = (): ConfigPayload => ({
  asr: { browser: { recognition_language: "ru-RU" } },
  logging: { full_enabled: false },
});

describe("config restart reasons", () => {
  it("does not require restart for full logging toggle (applied live on save)", () => {
    const previous = baseConfig();
    const next = {
      ...baseConfig(),
      logging: { full_enabled: true },
    };
    expect(getRestartRequiredReasons(previous, next)).toEqual([]);
  });

  it("detects web speech language change", () => {
    const previous = baseConfig();
    const next = {
      ...baseConfig(),
      asr: { browser: { recognition_language: "en-US" } },
    };
    expect(getRestartRequiredReasons(previous, next)).toEqual([
      "config.restart_reason.web_speech_language",
    ]);
  });

  it("builds restart warning when runtime is running", () => {
    const message = buildSaveStatusMessage(
      true,
      ["config.restart_reason.web_speech_language"],
      { running: true },
      "en",
    );
    expect(message).toContain("after Stop/Start");
  });
});
