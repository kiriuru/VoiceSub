import { describe, expect, it } from "vitest";

import {
  chunkTextForGoogleTts,
  GOOGLE_TTS_MAX_CHARS,
  normalizeGoogleTtsLang,
  textCharCount,
} from "./google-tts";

describe("normalizeGoogleTtsLang", () => {
  it("normalizes locale tags to base language", () => {
    expect(normalizeGoogleTtsLang("en-US")).toBe("en");
    expect(normalizeGoogleTtsLang("ru_RU")).toBe("ru");
    expect(normalizeGoogleTtsLang("")).toBe("en");
  });
});

describe("textCharCount", () => {
  it("counts Unicode scalars like Rust chars().count()", () => {
    expect(textCharCount("hello")).toBe(5);
    expect(textCharCount("привет")).toBe(6);
    expect(textCharCount("😀")).toBe(1);
  });
});

describe("chunkTextForGoogleTts", () => {
  it("returns empty for blank input", () => {
    expect(chunkTextForGoogleTts("   ")).toEqual([]);
  });

  it("keeps short text in one chunk", () => {
    expect(chunkTextForGoogleTts("hello world")).toEqual(["hello world"]);
  });

  it("splits long text near word boundaries", () => {
    const words = Array.from({ length: 60 }, (_, i) => `word${i}`).join(" ");
    const chunks = chunkTextForGoogleTts(words, GOOGLE_TTS_MAX_CHARS);
    expect(chunks.length).toBeGreaterThan(1);
    for (const chunk of chunks) {
      expect(textCharCount(chunk)).toBeLessThanOrEqual(GOOGLE_TTS_MAX_CHARS);
    }
  });
});
