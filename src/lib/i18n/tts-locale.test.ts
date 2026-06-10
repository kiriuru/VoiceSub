import { describe, expect, it } from "vitest";

import { setLocale, t } from "./index";

describe("TTS locale catalogs", () => {
  it("serves Japanese TTS strings", () => {
    setLocale("ja");
    expect(t("tts.module.title")).toBe("TTSモジュール");
    expect(t("tts.speech.sample_default")).toContain("VoiceSub");
  });

  it("serves Korean TTS strings", () => {
    setLocale("ko");
    expect(t("tts.module.title")).toBe("TTS 모듈");
  });

  it("serves Chinese TTS strings", () => {
    setLocale("zh");
    expect(t("tts.module.title")).toBe("TTS 模块");
  });
});
