import { describe, expect, it } from "vitest";
import en from "./i18n/locales/en.json";
import ja from "./i18n/locales/ja.json";
import ko from "./i18n/locales/ko.json";
import ru from "./i18n/locales/ru.json";
import zh from "./i18n/locales/zh.json";
import {
  BROWSER_RECOGNITION_LANGUAGE_CODES,
  BROWSER_RECOGNITION_LANGUAGES,
  LANGUAGES,
  TRANSLATION_LANGUAGE_CODES,
} from "./constants";

const localeCatalogs = { en, ru, ja, ko, zh };

describe("top-20 localization language lists", () => {
  it("exposes 20 translation target languages", () => {
    expect(LANGUAGES).toHaveLength(20);
    expect(TRANSLATION_LANGUAGE_CODES).toEqual([
      "en",
      "zh-cn",
      "zh-tw",
      "ru",
      "es",
      "pt",
      "de",
      "ko",
      "fr",
      "ja",
      "tr",
      "hi",
      "it",
      "ar",
      "pl",
      "id",
      "sv",
      "nl",
      "vi",
      "th",
    ]);
  });

  it("maps every translation language to an i18n label key in all UI locales", () => {
    for (const entry of LANGUAGES) {
      expect(entry.labelKey).toMatch(/^translation\.target_lang\./);
      for (const [locale, catalog] of Object.entries(localeCatalogs)) {
        expect(catalog[entry.labelKey], `${locale} missing ${entry.labelKey}`).toBeTruthy();
      }
    }
  });

  it("covers top-20 browser recognition locales with regional variants", () => {
    expect(BROWSER_RECOGNITION_LANGUAGES.length).toBeGreaterThanOrEqual(20);
    for (const code of [
      "en-US",
      "en-AU",
      "zh-CN",
      "zh-TW",
      "ru-RU",
      "es-ES",
      "pt-BR",
      "de-DE",
      "ko-KR",
      "fr-FR",
      "ja-JP",
      "tr-TR",
      "hi-IN",
      "it-IT",
      "ar-SA",
      "pl-PL",
      "id-ID",
      "sv-SE",
      "nl-NL",
      "vi-VN",
      "th-TH",
    ]) {
      expect(BROWSER_RECOGNITION_LANGUAGE_CODES).toContain(code);
    }
  });
});
