import {
  clearTwitchOAuthFragment,
  normalizeOAuthToken,
  parseTwitchAccessTokenFromLocation,
} from "./twitch-oauth";
import { isTauriWebview } from "./tauri-detect";

export async function tryCompleteExternalOAuthCallback(): Promise<boolean> {
  if (isTauriWebview()) return false;

  const raw = parseTwitchAccessTokenFromLocation();
  if (!raw) return false;

  const token = normalizeOAuthToken(raw);
  const response = await fetch("/api/tts/twitch/oauth-complete", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ token }),
  });
  clearTwitchOAuthFragment();

  return response.ok;
}
