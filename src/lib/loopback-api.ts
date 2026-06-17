export const LOOPBACK_TOKEN_HEADER = "x-voicesub-token";

declare global {
  interface Window {
    __VOICESUB_API_TOKEN__?: string;
  }
}

let cachedToken: string | null = null;

function readInjectedToken(): string | null {
  if (typeof window === "undefined") {
    return null;
  }
  const token = window.__VOICESUB_API_TOKEN__;
  return typeof token === "string" && token.trim() ? token.trim() : null;
}

export function loopbackApiToken(): string | null {
  return cachedToken || readInjectedToken();
}

export async function initLoopbackApiToken(): Promise<string | null> {
  const injected = readInjectedToken();
  if (injected) {
    cachedToken = injected;
    return injected;
  }
  if (cachedToken) {
    return cachedToken;
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const token = await invoke<string>("get_loopback_api_token");
    if (typeof token === "string" && token.trim()) {
      cachedToken = token.trim();
      return cachedToken;
    }
  } catch {
    // Browser dev without Tauri — rely on injected token from trusted HTML pages.
  }
  return null;
}

function requireLoopbackApiToken(): string {
  const token = loopbackApiToken();
  if (!token) {
    throw new Error(
      "VoiceSub loopback API token is missing. Reload the app or reopen the dashboard page.",
    );
  }
  return token;
}

export function loopbackApiHeaders(extra?: HeadersInit): HeadersInit {
  const headers = new Headers(extra);
  headers.set(LOOPBACK_TOKEN_HEADER, requireLoopbackApiToken());
  return headers;
}

export function withLoopbackAuth(init?: RequestInit): RequestInit {
  return {
    ...init,
    headers: loopbackApiHeaders(init?.headers),
  };
}

/** Clears cached token between Vitest cases (Tauri invoke / redirect races). */
export function __resetLoopbackApiTokenForTests(): void {
  cachedToken = null;
}
