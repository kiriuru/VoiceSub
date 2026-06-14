import { describe, expect, it } from "vitest";
import {
  prependSpeechActivity,
  speechActivityEntryFromQueueItem,
} from "./speech-activity-log";

describe("speechActivityEntryFromQueueItem", () => {
  it("maps subtitle source and translation slots", () => {
    expect(
      speechActivityEntryFromQueueItem({
        id: "sub-1-source-",
        text: "Привет",
        source: "subtitle_source",
        lang: "ru",
      }),
    ).toEqual({
      id: "sub-1-source-",
      lang: "ru",
      kind: "source",
      text: "Привет",
    });

    expect(
      speechActivityEntryFromQueueItem({
        id: "sub-1-translation-translation_1",
        text: "Hello",
        source: "subtitle_translation_1",
        lang: "en",
      }),
    ).toEqual({
      id: "sub-1-translation-translation_1",
      lang: "en",
      kind: "translation",
      slotId: "translation_1",
      text: "Hello",
    });
  });
});

describe("prependSpeechActivity", () => {
  it("prepends planned subtitle lines in order", () => {
    const next = prependSpeechActivity([], [
      {
        id: "a",
        text: "source line",
        source: "subtitle_source",
        lang: "ru",
      },
      {
        id: "b",
        text: "translation line",
        source: "subtitle_translation_2",
        lang: "de",
      },
    ]);
    expect(next).toHaveLength(2);
    expect(next[0]?.kind).toBe("source");
    expect(next[1]?.kind).toBe("translation");
  });
});
