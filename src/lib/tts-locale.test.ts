import { describe, expect, it } from "vitest";
import ttsEn from "./i18n/locales/tts-en.json";
import ttsJa from "./i18n/locales/tts-ja.json";
import ttsKo from "./i18n/locales/tts-ko.json";
import ttsRu from "./i18n/locales/tts-ru.json";
import ttsZh from "./i18n/locales/tts-zh.json";

describe("tts locale catalogs", () => {
  it("defines max_chars_hint in every tts locale without english fallback", () => {
    const catalogs = { en: ttsEn, ru: ttsRu, ja: ttsJa, ko: ttsKo, zh: ttsZh };
    for (const [locale, catalog] of Object.entries(catalogs)) {
      const hint = catalog["tts.twitch.max_chars_hint"];
      expect(hint, locale).toBeTruthy();
      if (locale !== "en") {
        expect(hint).not.toBe(ttsEn["tts.twitch.max_chars_hint"]);
      }
    }
  });
});
