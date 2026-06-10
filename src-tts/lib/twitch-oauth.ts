/** Minimal scope for IRC chat read (VoiceSub TTS does not send chat). */
export const TWITCH_CHAT_READ_SCOPE = "chat:read";

/**
 * VoiceSub-maintained Twitch Client ID (maintainer registers once at dev.twitch.tv).
 * When non-empty, end users click "Get new token" without registering their own app
 * (same model as BeatSaberPlus). Register redirect URIs:
 * `http://localhost:8765/tts` (+ your http.port if changed; use localhost, not 127.0.0.1).
 */
export const VOICESUB_TWITCH_OAUTH_CLIENT_ID = "oraf2d29s9mm8kxq4xx97zo28xaj7b";

const TWITCH_AUTHORIZE_URL = "https://id.twitch.tv/oauth2/authorize";

export function resolveTwitchClientId(settingsClientId?: string | null): string {
  const fromSettings = String(settingsClientId || "").trim();
  if (fromSettings) return fromSettings;
  return VOICESUB_TWITCH_OAUTH_CLIENT_ID.trim();
}

/** Use `localhost` hostname — Twitch allows http only for localhost, not 127.0.0.1. */
export function loopbackOAuthOrigin(port = 8765): string {
  return `http://localhost:${port}`;
}

/** Redirect URI for implicit grant — must match a URL registered in Twitch Developer Console. */
export function twitchOAuthRedirectUri(): string {
  if (typeof location === "undefined") {
    return `${loopbackOAuthOrigin()}/tts`;
  }
  const { protocol, hostname, port } = location;
  const host =
    hostname === "127.0.0.1" || hostname === "[::1]" ? "localhost" : hostname;
  const portSuffix =
    port && port !== "80" && port !== "443" ? `:${port}` : "";
  return `${protocol}//${host}${portSuffix}/tts`;
}

/**
 * BeatSaberPlus-style implicit OAuth URL (`response_type=token`).
 * Twitch redirects back to `/tts#access_token=…`; JS reads the hash fragment.
 */
export function buildTwitchAuthorizeUrl(clientId: string): string {
  const id = clientId.trim();
  if (!id) {
    throw new Error("Twitch Client ID is required");
  }
  const params = new URLSearchParams({
    client_id: id,
    redirect_uri: twitchOAuthRedirectUri(),
    response_type: "token",
    scope: TWITCH_CHAT_READ_SCOPE,
  });
  return `${TWITCH_AUTHORIZE_URL}?${params.toString()}`;
}

/** Parse `access_token` from URL hash after Twitch implicit redirect. */
export function parseTwitchAccessTokenFromLocation(href: string = location.href): string | null {
  const hashIndex = href.indexOf("#");
  if (hashIndex < 0) return null;
  const token = new URLSearchParams(href.slice(hashIndex + 1)).get("access_token")?.trim();
  return token || null;
}

export function clearTwitchOAuthFragment(): void {
  const url = location.href;
  const hashIndex = url.indexOf("#");
  if (hashIndex < 0) return;
  history.replaceState(null, document.title, url.slice(0, hashIndex));
}

export function normalizeOAuthToken(token: string): string {
  const trimmed = token.trim();
  if (!trimmed) return "";
  return trimmed.toLowerCase().startsWith("oauth:") ? trimmed : `oauth:${trimmed}`;
}
