import { apiFetch } from "./loopback-api-client";
import { ttsTrace } from "./tts-trace";
import type { TtsProvider } from "./types";

/** Google translate_tts practical per-request limit. */
export const GOOGLE_TTS_MAX_CHARS = 200;

/** Unicode scalar count — matches Rust `str.chars().count()` used by the TTS proxy. */
export function textCharCount(text: string): number {
  return Array.from(text).length;
}

export function normalizeGoogleTtsLang(lang: string | undefined | null): string {
  const trimmed = String(lang || "")
    .trim()
    .toLowerCase();
  if (!trimmed) return "en";
  return trimmed.split("-")[0]?.split("_")[0] || "en";
}

export function ttsProviderPath(provider: TtsProvider): string {
  return provider === "python_stdlib" ? "/api/tts/python" : "/api/tts/google";
}

/** Same-origin audio URL (Rust proxy or Python stdlib fetch). */
export function buildTtsAudioUrl(text: string, lang: string, provider: TtsProvider): string {
  const tl = normalizeGoogleTtsLang(lang);
  const q = encodeURIComponent(text);
  return `${ttsProviderPath(provider)}?tl=${tl}&q=${q}`;
}

/** Split long lines into Google-sized chunks (word boundaries when possible). */
export function chunkTextForGoogleTts(
  text: string,
  maxChars = GOOGLE_TTS_MAX_CHARS,
): string[] {
  const normalized = text.trim();
  if (!normalized) return [];
  const chars = Array.from(normalized);
  if (chars.length <= maxChars) return [normalized];

  const chunks: string[] = [];
  let start = 0;
  while (start < chars.length) {
    let end = Math.min(start + maxChars, chars.length);
    if (end < chars.length) {
      const minBreak = start + Math.floor(maxChars * 0.4);
      let spaceAt = -1;
      for (let index = end - 1; index >= minBreak; index -= 1) {
        if (chars[index] === " ") {
          spaceAt = index;
          break;
        }
      }
      if (spaceAt > start) {
        end = spaceAt;
      }
    }
    const piece = chars.slice(start, end).join("").trim();
    if (piece) chunks.push(piece);
    start = end;
    while (start < chars.length && chars[start] === " ") {
      start += 1;
    }
  }
  return chunks;
}

let ttsFetchWarmed = false;

/** Prime the local TTS proxy connection (TCP/TLS handshake) before first subtitle. */
export function warmupTtsFetch(lang = "en"): void {
  if (ttsFetchWarmed || typeof fetch === "undefined") return;
  ttsFetchWarmed = true;
  const url = buildTtsAudioUrl(".", lang, "browser_google");
  void apiFetch(url, { cache: "no-store", keepalive: true }).catch(() => {
    ttsFetchWarmed = false;
  });
  ttsTrace("google_tts", "warmup_started", { lang });
}
