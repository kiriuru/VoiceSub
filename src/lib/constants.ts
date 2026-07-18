export const PROVIDERS = {
  google_translate_v2: {
    label: "Google Translate v2",
    group: "Stable / Recommended",
    fields: ["api_key"] as const,
    setupUrl: "https://console.cloud.google.com/apis/credentials",
  },
  google_cloud_translation_v3: {
    label: "Google Cloud Translation v3",
    group: "Stable / Recommended",
    fields: ["api_key", "endpoint", "region", "model"] as const,
    apiKeyPlaceholder: "OAuth access token",
    endpointPlaceholder: "my-gcp-project",
    regionPlaceholder: "global",
    modelPlaceholder: "general/nmt (expanded to projects/…/models/…)",
    setupUrl: "https://console.cloud.google.com/apis/api/translate.googleapis.com",
  },
  google_gas_url: {
    label: "Google GAS URL",
    group: "Experimental / Emergency",
    fields: ["gas_url"] as const,
    setupUrl: "https://script.google.com/",
  },
  google_web: {
    label: "Google Web",
    group: "Experimental / Emergency",
    fields: [] as const,
  },
  azure_translator: {
    label: "Azure Translator",
    group: "Stable / Recommended",
    fields: ["api_key", "endpoint", "region"] as const,
    endpointPlaceholder: "https://api.cognitive.microsofttranslator.com",
    setupUrl: "https://portal.azure.com/#view/Microsoft_Azure_ProjectOxford/CognitiveServicesHub/~/TextTranslation",
  },
  deepl: {
    label: "DeepL",
    group: "Classic MT",
    fields: ["api_key", "api_url"] as const,
    apiUrlPlaceholder: "https://api-free.deepl.com/v2/translate",
    setupUrl: "https://www.deepl.com/pro-api",
  },
  libretranslate: {
    label: "LibreTranslate",
    group: "Classic MT",
    fields: ["api_key", "api_url"] as const,
    apiUrlPlaceholder: "https://libretranslate.com/translate",
    setupUrl: "https://libretranslate.com",
  },
  openai: {
    label: "OpenAI",
    group: "Flexible LLM",
    fields: ["api_key", "base_url", "model", "custom_prompt"] as const,
    baseUrlPlaceholder: "https://api.openai.com/v1",
    setupUrl: "https://platform.openai.com/api-keys",
  },
  openrouter: {
    label: "OpenRouter",
    group: "Flexible LLM",
    fields: ["api_key", "base_url", "model", "custom_prompt"] as const,
    baseUrlPlaceholder: "https://openrouter.ai/api/v1",
    setupUrl: "https://openrouter.ai/keys",
  },
  lm_studio: {
    label: "LM Studio",
    group: "Local LLM",
    fields: ["base_url", "model", "custom_prompt"] as const,
    baseUrlPlaceholder: "http://127.0.0.1:1234/v1",
  },
  ollama: {
    label: "Ollama",
    group: "Local LLM",
    fields: ["base_url", "model", "custom_prompt"] as const,
    baseUrlPlaceholder: "http://127.0.0.1:11434/v1",
  },
  public_libretranslate_mirror: {
    label: "Public LibreTranslate Mirror",
    group: "Experimental / Emergency",
    fields: ["api_url"] as const,
    apiUrlPlaceholder: "https://translate.fedilab.app/translate",
  },
  free_web_translate: {
    label: "Free Web Translate",
    group: "Experimental / Emergency",
    fields: [] as const,
  },
  baidu_translate: {
    label: "Baidu Translate",
    group: "China / Free-tier",
    fields: ["app_id", "secret_key"] as const,
    setupUrl: "https://fanyi-api.baidu.com/",
  },
  youdao_translate: {
    label: "Youdao Translate",
    group: "China / Free-tier",
    fields: ["app_key", "app_secret"] as const,
    setupUrl: "https://ai.youdao.com/",
  },
  tencent_tmt: {
    label: "Tencent Machine Translation",
    group: "China / Free-tier",
    fields: ["secret_id", "secret_key", "region"] as const,
    regionPlaceholder: "ap-guangzhou",
    setupUrl: "https://console.cloud.tencent.com/tmt",
  },
  caiyun_translator: {
    label: "Caiyun Xiaoyi",
    group: "China / Free-tier",
    fields: ["token"] as const,
    setupUrl: "https://fanyi.caiyunapp.com/",
  },
} as const;

/** Official OpenAI chat IDs used when “Show all” is off (kept in sync with runtime catalog). */
export const OPENAI_RECOMMENDED_CHAT_MODELS = [
  "gpt-5.6-luna",
  "gpt-5.6-terra",
  "gpt-5.6-sol",
  "gpt-5.6",
  "gpt-5.4-nano",
  "gpt-5.4-mini",
  "gpt-5.4",
  "gpt-5-nano",
  "gpt-5-mini",
  "gpt-5",
  "gpt-4.1-nano",
  "gpt-4.1-mini",
  "gpt-4.1",
  "gpt-4o-mini",
  "gpt-4o",
] as const;

export type ProviderId = keyof typeof PROVIDERS;

export const PROVIDER_GROUP_I18N_KEYS: Record<string, string> = {
  "Stable / Recommended": "translation.provider_group.stable_recommended",
  "Experimental / Emergency": "translation.provider_group.experimental_emergency",
  "Classic MT": "translation.provider_group.classic_mt",
  "Flexible LLM": "translation.provider_group.flexible_llm",
  "Local LLM": "translation.provider_group.local_llm",
  "China / Free-tier": "translation.provider_group.china_free_tier",
};

/** Top-20 localization targets for translation (Steam/Twitch 2025). */
export const LANGUAGES = [
  { code: "en", labelKey: "translation.target_lang.en" },
  { code: "zh-cn", labelKey: "translation.target_lang.zh_cn" },
  { code: "zh-tw", labelKey: "translation.target_lang.zh_tw" },
  { code: "ru", labelKey: "translation.target_lang.ru" },
  { code: "es", labelKey: "translation.target_lang.es" },
  { code: "pt", labelKey: "translation.target_lang.pt" },
  { code: "de", labelKey: "translation.target_lang.de" },
  { code: "ko", labelKey: "translation.target_lang.ko" },
  { code: "fr", labelKey: "translation.target_lang.fr" },
  { code: "ja", labelKey: "translation.target_lang.ja" },
  { code: "tr", labelKey: "translation.target_lang.tr" },
  { code: "hi", labelKey: "translation.target_lang.hi" },
  { code: "it", labelKey: "translation.target_lang.it" },
  { code: "ar", labelKey: "translation.target_lang.ar" },
  { code: "pl", labelKey: "translation.target_lang.pl" },
  { code: "id", labelKey: "translation.target_lang.id" },
  { code: "sv", labelKey: "translation.target_lang.sv" },
  { code: "nl", labelKey: "translation.target_lang.nl" },
  { code: "vi", labelKey: "translation.target_lang.vi" },
  { code: "th", labelKey: "translation.target_lang.th" },
] as const;

export const TRANSLATION_LANGUAGE_CODES = LANGUAGES.map((entry) => entry.code);

/**
 * Web Speech API (Chrome worker) recognition locales for the top-20 set.
 * Regional variants follow the localization list (en-US/en-GB, es-ES/es-MX, zh-CN/zh-TW).
 * uk-UA is kept for existing configs and Eastern European streams.
 */
export const BROWSER_RECOGNITION_LANGUAGES = [
  { code: "en-US", label: "English (en-US)" },
  { code: "en-GB", label: "English (en-GB)" },
  { code: "en-AU", label: "English Australia (en-AU)" },
  { code: "zh-CN", label: "Chinese Simplified (zh-CN)" },
  { code: "zh-TW", label: "Chinese Traditional (zh-TW)" },
  { code: "ru-RU", label: "Russian (ru-RU)" },
  { code: "es-ES", label: "Spanish (es-ES)" },
  { code: "es-MX", label: "Spanish (es-MX)" },
  { code: "pt-BR", label: "Portuguese (pt-BR)" },
  { code: "de-DE", label: "German (de-DE)" },
  { code: "ko-KR", label: "Korean (ko-KR)" },
  { code: "fr-FR", label: "French (fr-FR)" },
  { code: "ja-JP", label: "Japanese (ja-JP)" },
  { code: "tr-TR", label: "Turkish (tr-TR)" },
  { code: "hi-IN", label: "Hindi (hi-IN)" },
  { code: "it-IT", label: "Italian (it-IT)" },
  { code: "ar-SA", label: "Arabic (ar-SA)" },
  { code: "pl-PL", label: "Polish (pl-PL)" },
  { code: "id-ID", label: "Indonesian (id-ID)" },
  { code: "sv-SE", label: "Swedish (sv-SE)" },
  { code: "nl-NL", label: "Dutch (nl-NL)" },
  { code: "vi-VN", label: "Vietnamese (vi-VN)" },
  { code: "th-TH", label: "Thai (th-TH)" },
  { code: "uk-UA", label: "Ukrainian (uk-UA)" },
] as const;

export const BROWSER_RECOGNITION_LANGUAGE_CODES = BROWSER_RECOGNITION_LANGUAGES.map(
  (entry) => entry.code
);

export const OVERLAY_PRESETS = ["single", "dual-line", "stacked", "compact"] as const;

export const UI_LOCALES = [
  { code: "en", labelKey: "language.en" },
  { code: "ru", labelKey: "language.ru" },
  { code: "ja", labelKey: "language.ja" },
  { code: "ko", labelKey: "language.ko" },
  { code: "zh", labelKey: "language.zh" },
] as const;
