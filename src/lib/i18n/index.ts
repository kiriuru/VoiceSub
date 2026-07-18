import { writable, get } from "svelte/store";
import { publishUiLocaleSync } from "../ui-config-sync";
import type { LocaleCode } from "../types";

export type { LocaleCode };
import en from "./locales/en.json";
import ru from "./locales/ru.json";
import ja from "./locales/ja.json";
import ko from "./locales/ko.json";
import zh from "./locales/zh.json";
import ttsEn from "./locales/tts-en.json";
import ttsRu from "./locales/tts-ru.json";
import ttsJa from "./locales/tts-ja.json";
import ttsKo from "./locales/tts-ko.json";
import ttsZh from "./locales/tts-zh.json";

const catalogs: Record<LocaleCode, Record<string, string>> = {
  en: { ...en, ...ttsEn },
  ru: { ...ru, ...ttsRu },
  ja: { ...ja, ...ttsJa },
  ko: { ...ko, ...ttsKo },
  zh: { ...zh, ...ttsZh },
};

function detectLocale(): LocaleCode {
  const stored = localStorage.getItem("voicesub.ui.locale");
  if (stored && stored in catalogs) return stored as LocaleCode;
  const nav = (navigator.language || "en").slice(0, 2).toLowerCase();
  if (nav in catalogs) return nav as LocaleCode;
  return "en";
}

export const locale = writable<LocaleCode>(detectLocale());

locale.subscribe((code) => {
  document.documentElement.lang = code;
  localStorage.setItem("voicesub.ui.locale", code);
});

type Interpolation = Record<string, string | number | boolean | null | undefined>;

export function getLocale(): LocaleCode {
  return get(locale);
}

export function setLocale(code: LocaleCode): void {
  if (get(locale) === code) {
    return;
  }
  locale.set(code);
  publishUiLocaleSync(code);
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent("sst:locale-changed", { detail: { locale: code } }));
  }
}

export function t(key: string, vars?: Interpolation, code?: LocaleCode): string {
  const lang = code ?? get(locale);
  const catalog = catalogs[lang] ?? catalogs.en;
  let text = catalog[key] ?? catalogs.en[key] ?? key;
  if (vars) {
    for (const [name, value] of Object.entries(vars)) {
      text = text.replaceAll(`{${name}}`, String(value ?? ""));
    }
  }
  return text;
}
