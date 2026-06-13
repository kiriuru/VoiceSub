import { describe, expect, it } from "vitest";

import {
  chunkTextForGoogleTts,
  GOOGLE_TTS_MAX_CHARS,
  normalizeGoogleTtsLang,
  textCharCount,
  waitForPreparedChunk,
  type PreparedGoogleTts,
  type PreparedGoogleTtsChunk,
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

describe("textCharCount", () => {
  it("counts emoji as one character", () => {
    expect(textCharCount("😀")).toBe(1);
    expect(textCharCount("a😀b")).toBe(3);
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
      expect(textCharCount(chunk)).toBeLessThanOrEqual(GOOGLE_TTS_MAX_CHARS);
    }
    expect(chunks.join(" ")).toContain("word");
  });

  it("keeps emoji code points intact within chunk limits", () => {
    const emoji = "😀".repeat(250);
    const chunks = chunkTextForGoogleTts(emoji);
    expect(chunks.length).toBeGreaterThan(1);
    for (const chunk of chunks) {
      expect(textCharCount(chunk)).toBeLessThanOrEqual(GOOGLE_TTS_MAX_CHARS);
      expect(chunk.includes("\uD800")).toBe(false);
    }
    expect(chunks.join("")).toBe(emoji);
  });

  it("returns empty for whitespace-only input", () => {
    expect(chunkTextForGoogleTts("   ")).toEqual([]);
  });
});

describe("waitForPreparedChunk", () => {
  it("returns an already-ready chunk without waiting", async () => {
    const chunk: PreparedGoogleTtsChunk = {
      data: new Uint8Array([0x49, 0x44, 0x33]),
      bytes: 3,
      provider: "browser_google",
      textLen: 1,
    };
    const prepared: PreparedGoogleTts = {
      chunks: [chunk],
      expectedChunkCount: 1,
      readyCount: 1,
    };
    await expect(waitForPreparedChunk(prepared, 0)).resolves.toBe(chunk);
  });

  it("fails fast when prefetchError is set", async () => {
    const prepared: PreparedGoogleTts = {
      chunks: [undefined, undefined],
      expectedChunkCount: 2,
      readyCount: 2,
      prefetchError: "network down",
    };
    await expect(waitForPreparedChunk(prepared, 1)).rejects.toThrow("network down");
  });
});
