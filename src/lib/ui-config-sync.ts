import type { ConfigPayload, LocaleCode } from "./types";

export const UI_CONFIG_CHANNEL = "voicesub:ui-config";
export const UI_LOCALE_CHANNEL = "voicesub:ui-locale";

type UiConfigSyncMessage = {
  type: "ui_config";
  payload: ConfigPayload;
};

type UiLocaleSyncMessage = {
  type: "ui_locale";
  locale: LocaleCode;
};

const SUPPORTED_LOCALES = new Set<LocaleCode>(["en", "ru", "ja", "ko", "zh"]);

function isSupportedLocale(value: string): value is LocaleCode {
  return SUPPORTED_LOCALES.has(value as LocaleCode);
}

/** Push live UI locale to other VoiceSub windows (TTS module, etc.). */
export function publishUiLocaleSync(locale: LocaleCode): void {
  if (typeof BroadcastChannel === "undefined") return;
  try {
    const channel = new BroadcastChannel(UI_LOCALE_CHANNEL);
    const message: UiLocaleSyncMessage = { type: "ui_locale", locale };
    channel.postMessage(message);
    channel.close();
  } catch {
    // optional cross-window sync
  }
}

export function subscribeUiLocaleSync(handler: (locale: LocaleCode) => void): () => void {
  if (typeof BroadcastChannel === "undefined") {
    return () => {};
  }
  try {
    const channel = new BroadcastChannel(UI_LOCALE_CHANNEL);
    channel.onmessage = (event: MessageEvent<UiLocaleSyncMessage>) => {
      const next = event.data?.locale;
      if (event.data?.type === "ui_locale" && next && isSupportedLocale(next)) {
        handler(next);
      }
    };
    return () => channel.close();
  } catch {
    return () => {};
  }
}

/** Push live dashboard config to other VoiceSub windows (TTS module, etc.). */
export function publishUiConfigSync(payload: ConfigPayload): void {
  if (typeof BroadcastChannel === "undefined") return;
  try {
    const channel = new BroadcastChannel(UI_CONFIG_CHANNEL);
    const message: UiConfigSyncMessage = { type: "ui_config", payload };
    channel.postMessage(message);
    channel.close();
  } catch {
    // optional cross-window sync
  }
}

export function subscribeUiConfigSync(
  handler: (payload: ConfigPayload) => void,
): () => void {
  if (typeof BroadcastChannel === "undefined") {
    return () => {};
  }
  try {
    const channel = new BroadcastChannel(UI_CONFIG_CHANNEL);
    channel.onmessage = (event: MessageEvent<UiConfigSyncMessage>) => {
      if (event.data?.type === "ui_config" && event.data.payload) {
        handler(event.data.payload);
      }
    };
    return () => channel.close();
  } catch {
    return () => {};
  }
}
