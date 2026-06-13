import type { ConfigPayload, LocaleCode } from "./types";

export const UI_CONFIG_CHANNEL = "voicesub:ui-config";
export const UI_LOCALE_CHANNEL = "voicesub:ui-locale";
export const UI_CONFIG_WS_EVENT = "ui_config_sync";

const UI_CONFIG_SERVER_SYNC_DEBOUNCE_MS = 120;

type UiConfigSyncMessage = {
  type: "ui_config";
  payload: ConfigPayload;
};

type UiLocaleSyncMessage = {
  type: "ui_locale";
  locale: LocaleCode;
};

const SUPPORTED_LOCALES = new Set<LocaleCode>(["en", "ru", "ja", "ko", "zh"]);

let uiConfigServerSyncTimer: ReturnType<typeof setTimeout> | null = null;
let uiConfigServerSyncPayload: ConfigPayload | null = null;

function isSupportedLocale(value: string): value is LocaleCode {
  return SUPPORTED_LOCALES.has(value as LocaleCode);
}

export function uiConfigFromWsPayload(raw: unknown): ConfigPayload | null {
  if (!raw || typeof raw !== "object") return null;
  const ui = (raw as Record<string, unknown>).ui;
  if (!ui || typeof ui !== "object") return null;
  return { ui: ui as ConfigPayload["ui"] };
}

function publishUiConfigBroadcastChannel(payload: ConfigPayload): void {
  if (typeof BroadcastChannel === "undefined") return;
  try {
    const channel = new BroadcastChannel(UI_CONFIG_CHANNEL);
    const message: UiConfigSyncMessage = { type: "ui_config", payload };
    channel.postMessage(message);
    channel.close();
  } catch {
    // optional same-browser-context sync
  }
}

function scheduleUiConfigServerSync(payload: ConfigPayload): void {
  uiConfigServerSyncPayload = payload;
  if (uiConfigServerSyncTimer) {
    clearTimeout(uiConfigServerSyncTimer);
  }
  uiConfigServerSyncTimer = setTimeout(() => {
    uiConfigServerSyncTimer = null;
    const next = uiConfigServerSyncPayload;
    uiConfigServerSyncPayload = null;
    if (!next) return;
    void pushUiConfigToServer(next);
  }, UI_CONFIG_SERVER_SYNC_DEBOUNCE_MS);
}

async function pushUiConfigToServer(payload: ConfigPayload): Promise<void> {
  try {
    await fetch("/api/ui/sync", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ ui: payload.ui || {} }),
    });
  } catch {
    // best-effort cross-window fan-out
  }
}

function subscribeUiConfigBroadcastChannel(handler: (payload: ConfigPayload) => void): () => void {
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

function buildUiConfigWsUrl(): string {
  const protocol = location.protocol === "https:" ? "wss" : "ws";
  return `${protocol}://${location.host}/ws/events`;
}

function subscribeUiConfigWebSocket(handler: (payload: ConfigPayload) => void): () => void {
  if (typeof WebSocket === "undefined") {
    return () => {};
  }

  let socket: WebSocket | null = null;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  let manualClose = false;
  let backoffMs = 1000;

  const connect = () => {
    if (manualClose) return;
    if (socket?.readyState === WebSocket.OPEN || socket?.readyState === WebSocket.CONNECTING) {
      return;
    }

    socket = new WebSocket(buildUiConfigWsUrl());

    socket.addEventListener("open", () => {
      backoffMs = 1000;
      socket?.send("ping");
    });

    socket.addEventListener("message", (event) => {
      try {
        const message = JSON.parse(String(event.data)) as { type?: string; payload?: unknown };
        if (message.type !== UI_CONFIG_WS_EVENT) return;
        const payload = uiConfigFromWsPayload(message.payload);
        if (payload) handler(payload);
      } catch {
        // ignore non-json frames
      }
    });

    socket.addEventListener("close", () => {
      socket = null;
      if (manualClose) return;
      reconnectTimer = setTimeout(() => {
        reconnectTimer = null;
        connect();
      }, backoffMs);
      backoffMs = Math.min(10000, backoffMs * 2);
    });

    socket.addEventListener("error", () => {
      socket?.close();
    });
  };

  connect();

  return () => {
    manualClose = true;
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    socket?.close();
    socket = null;
  };
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

/** Push live dashboard UI config to worker/TTS windows without persisting settings. */
export function publishUiConfigSync(payload: ConfigPayload): void {
  publishUiConfigBroadcastChannel(payload);
  scheduleUiConfigServerSync(payload);
}

export function subscribeUiConfigSync(
  handler: (payload: ConfigPayload) => void,
  options?: { enableWebSocket?: boolean },
): () => void {
  const unsubs = [subscribeUiConfigBroadcastChannel(handler)];
  if (options?.enableWebSocket !== false) {
    unsubs.push(subscribeUiConfigWebSocket(handler));
  }
  return () => {
    for (const unsub of unsubs) unsub();
  };
}

export function mergeUiConfigPatch(base: ConfigPayload, partial: ConfigPayload): ConfigPayload {
  return {
    ...base,
    ...partial,
    ui: { ...(base.ui || {}), ...(partial.ui || {}) },
  };
}

/** @internal Test helper — flush debounced server sync immediately. */
export function flushUiConfigServerSyncForTests(): void {
  if (uiConfigServerSyncTimer) {
    clearTimeout(uiConfigServerSyncTimer);
    uiConfigServerSyncTimer = null;
  }
  const next = uiConfigServerSyncPayload;
  uiConfigServerSyncPayload = null;
  if (next) {
    void pushUiConfigToServer(next);
  }
}
