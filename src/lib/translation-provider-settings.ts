import { PROVIDERS, type ProviderId } from "./constants";

export type ProviderSettingsMap = Record<string, Record<string, string>>;

function str(value: unknown): string {
  return String(value ?? "");
}

/** SST `normalize_provider_settings` parity for dashboard save/load. */
export function defaultTranslationProviderSettings(): ProviderSettingsMap {
  return {
    google_translate_v2: { api_key: "" },
    google_cloud_translation_v3: {
      project_id: "",
      access_token: "",
      location: "global",
      model: "",
    },
    google_gas_url: { gas_url: "" },
    google_web: {},
    azure_translator: {
      api_key: "",
      endpoint: "https://api.cognitive.microsofttranslator.com",
      region: "",
    },
    deepl: {
      api_key: "",
      api_url: "https://api-free.deepl.com/v2/translate",
    },
    libretranslate: {
      api_key: "",
      api_url: "https://libretranslate.com/translate",
    },
    openai: {
      api_key: "",
      base_url: "https://api.openai.com/v1",
      model: "",
      custom_prompt: "",
    },
    openrouter: {
      api_key: "",
      base_url: "https://openrouter.ai/api/v1",
      model: "",
      custom_prompt: "",
    },
    lm_studio: {
      api_key: "",
      base_url: "http://127.0.0.1:1234/v1",
      model: "",
      custom_prompt: "",
    },
    ollama: {
      api_key: "",
      base_url: "http://127.0.0.1:11434/v1",
      model: "",
      custom_prompt: "",
    },
    public_libretranslate_mirror: {
      api_url: "https://translate.fedilab.app/translate",
    },
    free_web_translate: {},
  };
}

export function normalizeTranslationProviderSettings(
  raw: Record<string, unknown> | undefined,
): ProviderSettingsMap {
  const defaults = defaultTranslationProviderSettings();
  const payload = raw && typeof raw === "object" ? raw : {};
  const normalized: ProviderSettingsMap = {};

  for (const providerName of Object.keys(PROVIDERS) as ProviderId[]) {
    const providerDefaults = defaults[providerName] ?? {};
    const current =
      payload[providerName] && typeof payload[providerName] === "object"
        ? (payload[providerName] as Record<string, unknown>)
        : {};

    if (providerName === "google_cloud_translation_v3") {
      const accessToken = str(
        current.access_token ?? current.api_key ?? providerDefaults.access_token ?? "",
      );
      const projectId = str(current.project_id ?? current.endpoint ?? providerDefaults.project_id ?? "");
      const location = str(
        current.location ?? current.region ?? providerDefaults.location ?? "global",
      );
      normalized[providerName] = {
        project_id: projectId,
        access_token: accessToken,
        location: location || "global",
        model: str(current.model ?? providerDefaults.model ?? ""),
      };
      continue;
    }

    if (providerName === "google_web" || providerName === "free_web_translate") {
      normalized[providerName] = {};
      continue;
    }

    const next: Record<string, string> = {};
    for (const [key, fallback] of Object.entries(providerDefaults)) {
      const value = current[key];
      next[key] = str(value ?? fallback);
    }

    if (providerName === "azure_translator") {
      next.endpoint =
        str(current.endpoint) || "https://api.cognitive.microsofttranslator.com";
    }
    if (providerName === "deepl") {
      next.api_url = str(current.api_url) || PROVIDERS.deepl.apiUrlPlaceholder;
    }
    if (providerName === "libretranslate") {
      next.api_url = str(current.api_url) || PROVIDERS.libretranslate.apiUrlPlaceholder;
    }
    if (providerName === "openai") {
      next.base_url = str(current.base_url) || PROVIDERS.openai.baseUrlPlaceholder;
    }
    if (providerName === "openrouter") {
      next.base_url = str(current.base_url) || PROVIDERS.openrouter.baseUrlPlaceholder;
    }
    if (providerName === "lm_studio") {
      next.base_url = str(current.base_url) || PROVIDERS.lm_studio.baseUrlPlaceholder;
    }
    if (providerName === "ollama") {
      next.base_url = str(current.base_url) || PROVIDERS.ollama.baseUrlPlaceholder;
    }
    if (providerName === "public_libretranslate_mirror") {
      next.api_url =
        str(current.api_url) || PROVIDERS.public_libretranslate_mirror.apiUrlPlaceholder;
    }

    normalized[providerName] = next;
  }

  return normalized;
}
