import { describe, expect, it } from "vitest";

import {
  chunkTextForGoogleTts,
  GOOGLE_TTS_MAX_CHARS,
  normalizeGoogleTtsLang,
} from "./google-tts";

describe("normalizeGoogleTtsLang", () => {
  it("strips region subtags", () => {
    expect(normalizeGoogleTtsLang("ru-RU")).toBe("ru");
    expect(normalizeGoogleTtsLang("en_US")).toBe("en");
  });

  it("defaults empty to en", () => {
    expect(normalizeGoogleTtsLang("")).toBe("en");
    expect(normalizeGoogleTtsLang(null)).toBe("en");
  });
});

describe("chunkTextForGoogleTts", () => {
  it("returns a single chunk for short text", () => {
    expect(chunkTextForGoogleTts("Hello")).toEqual(["Hello"]);
  });

  it("splits long text into google-sized chunks", () => {
    const text = "word ".repeat(80).trim();
    const chunks = chunkTextForGoogleTts(text);
    expect(chunks.length).toBeGreaterThan(1);
    for (const chunk of chunks) {
      expect(chunk.length).toBeLessThanOrEqual(GOOGLE_TTS_MAX_CHARS);
    }
    expect(chunks.join(" ")).toContain("word");
  });

  it("returns empty for whitespace-only input", () => {
    expect(chunkTextForGoogleTts("   ")).toEqual([]);
  });
});
