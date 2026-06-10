export const PROVIDERS = {
  google_translate_v2: {
    label: "Google Translate v2",
    group: "Stable / Recommended",
    fields: ["api_key"] as const,
  },
  google_cloud_translation_v3: {
    label: "Google Cloud Translation v3",
    group: "Stable / Recommended",
    fields: ["api_key", "endpoint", "region", "model"] as const,
    apiKeyPlaceholder: "OAuth access token",
    endpointPlaceholder: "my-gcp-project",
    regionPlaceholder: "global",
    modelPlaceholder: "general/nmt or general/translation-llm",
  },
  google_gas_url: {
    label: "Google GAS URL",
    group: "Experimental / Emergency",
    fields: ["gas_url"] as const,
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
  },
  deepl: {
    label: "DeepL",
    group: "Classic MT",
    fields: ["api_key", "api_url"] as const,
    apiUrlPlaceholder: "https://api-free.deepl.com/v2/translate",
  },
  libretranslate: {
    label: "LibreTranslate",
    group: "Classic MT",
    fields: ["api_key", "api_url"] as const,
    apiUrlPlaceholder: "https://libretranslate.com/translate",
  },
  openai: {
    label: "OpenAI",
    group: "Flexible LLM",
    fields: ["api_key", "base_url", "model", "custom_prompt"] as const,
    baseUrlPlaceholder: "https://api.openai.com/v1",
  },
  openrouter: {
    label: "OpenRouter",
    group: "Flexible LLM",
    fields: ["api_key", "base_url", "model", "custom_prompt"] as const,
    baseUrlPlaceholder: "https://openrouter.ai/api/v1",
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
} as const;

export type ProviderId = keyof typeof PROVIDERS;

export const PROVIDER_GROUP_I18N_KEYS: Record<string, string> = {
  "Stable / Recommended": "translation.provider_group.stable_recommended",
  "Experimental / Emergency": "translation.provider_group.experimental_emergency",
  "Classic MT": "translation.provider_group.classic_mt",
  "Flexible LLM": "translation.provider_group.flexible_llm",
  "Local LLM": "translation.provider_group.local_llm",
};

export const LANGUAGES = [
  { code: "en", label: "English" },
  { code: "ja", label: "Japanese" },
  { code: "de", label: "German" },
  { code: "es", label: "Spanish" },
  { code: "fr", label: "French" },
  { code: "it", label: "Italian" },
  { code: "ko", label: "Korean" },
  { code: "pt", label: "Portuguese" },
  { code: "ru", label: "Russian" },
  { code: "zh-cn", label: "Chinese (Simplified)" },
];

export const BROWSER_RECOGNITION_LANGUAGES = [
  { code: "ru-RU", label: "Russian (ru-RU)" },
  { code: "en-US", label: "English (en-US)" },
  { code: "en-AU", label: "English Australia (en-AU)" },
  { code: "de-DE", label: "German (de-DE)" },
  { code: "es-ES", label: "Spanish (es-ES)" },
  { code: "fr-FR", label: "French (fr-FR)" },
  { code: "it-IT", label: "Italian (it-IT)" },
  { code: "ja-JP", label: "Japanese (ja-JP)" },
  { code: "ko-KR", label: "Korean (ko-KR)" },
  { code: "pl-PL", label: "Polish (pl-PL)" },
  { code: "pt-BR", label: "Portuguese (pt-BR)" },
  { code: "uk-UA", label: "Ukrainian (uk-UA)" },
  { code: "zh-CN", label: "Chinese Simplified (zh-CN)" },
];

export const OVERLAY_PRESETS = ["single", "dual-line", "stacked", "compact"] as const;

export const UI_LOCALES = [
  { code: "en", labelKey: "language.en" },
  { code: "ru", labelKey: "language.ru" },
  { code: "ja", labelKey: "language.ja" },
  { code: "ko", labelKey: "language.ko" },
  { code: "zh", labelKey: "language.zh" },
] as const;
