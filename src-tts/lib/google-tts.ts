import { ttsTrace } from "./tts-trace";
import type { TtsProvider } from "./types";

/** Google translate_tts practical per-request limit. */
export const GOOGLE_TTS_MAX_CHARS = 200;

/** Abort a stuck chunk so the speech queue can advance. */
export const GOOGLE_TTS_PLAYBACK_TIMEOUT_MS = 45_000;

const MEDIA_ERROR_LABELS = [
  "",
  "MEDIA_ERR_ABORTED",
  "MEDIA_ERR_NETWORK",
  "MEDIA_ERR_DECODE",
  "MEDIA_ERR_SRC_NOT_SUPPORTED",
] as const;

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
  if (normalized.length <= maxChars) return [normalized];

  const chunks: string[] = [];
  let rest = normalized;
  while (rest.length > maxChars) {
    let cut = rest.lastIndexOf(" ", maxChars);
    if (cut < Math.floor(maxChars * 0.4)) {
      cut = maxChars;
    }
    const piece = rest.slice(0, cut).trim();
    if (piece) chunks.push(piece);
    rest = rest.slice(cut).trim();
  }
  if (rest) chunks.push(rest);
  return chunks;
}

export type GoogleTtsPlaybackOptions = {
  lang: string;
  provider?: TtsProvider;
  rate?: number;
  volume?: number;
  sinkId?: string;
  onAudio?: (audio: HTMLAudioElement) => void;
};

function clampRate(rate: number | undefined): number {
  if (typeof rate !== "number" || !Number.isFinite(rate)) return 1;
  return Math.min(2, Math.max(0.5, rate));
}

function setPreservesPitch(audio: HTMLAudioElement, enabled: boolean): void {
  if ("preservesPitch" in audio) {
    audio.preservesPitch = enabled;
  }
  const legacy = audio as HTMLAudioElement & {
    mozPreservesPitch?: boolean;
    webkitPreservesPitch?: boolean;
  };
  if ("mozPreservesPitch" in legacy) {
    legacy.mozPreservesPitch = enabled;
  }
  if ("webkitPreservesPitch" in legacy) {
    legacy.webkitPreservesPitch = enabled;
  }
}

export type GoogleTtsPrefetchOptions = {
  lang: string;
  provider?: TtsProvider;
};

export type PreparedGoogleTtsChunk = {
  blob: Blob;
  bytes: number;
  provider: TtsProvider;
  textLen: number;
};

export type PreparedGoogleTts = {
  chunks: PreparedGoogleTtsChunk[];
};

function mediaErrorFields(audio: HTMLAudioElement): Record<string, unknown> {
  const code = audio.error?.code ?? null;
  const label =
    typeof code === "number" && code >= 0 && code < MEDIA_ERROR_LABELS.length
      ? MEDIA_ERROR_LABELS[code]
      : code != null
        ? `code_${code}`
        : "unknown";
  return {
    media_error_code: code,
    media_error_label: label,
    media_error_message: audio.error?.message || "",
    network_state: audio.networkState,
    ready_state: audio.readyState,
  };
}

function formatMediaError(audio: HTMLAudioElement): string {
  const fields = mediaErrorFields(audio);
  const label = String(fields.media_error_label || "unknown");
  const detail = String(fields.media_error_message || label);
  return `Google TTS media error: ${detail} (${label})`;
}

function looksLikeMpegAudio(bytes: Uint8Array): boolean {
  if (bytes.length < 2) return false;
  if (bytes[0] === 0x49 && bytes[1] === 0x44 && bytes[2] === 0x33) return true;
  return bytes[0] === 0xff && (bytes[1] & 0xe0) === 0xe0;
}

async function fetchTtsAudioBlob(
  text: string,
  lang: string,
  provider: TtsProvider,
): Promise<{ blob: Blob; provider: TtsProvider; bytes: number }> {
  const url = buildTtsAudioUrl(text, lang, provider);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(
      `TTS HTTP ${response.status} from ${ttsProviderPath(provider)}`,
    );
  }
  const buffer = await response.arrayBuffer();
  const bytes = new Uint8Array(buffer);
  if (!looksLikeMpegAudio(bytes)) {
    throw new Error(
      `TTS response is not MPEG audio (${bytes.length} bytes from ${ttsProviderPath(provider)})`,
    );
  }
  return {
    blob: new Blob([bytes], { type: "audio/mpeg" }),
    provider,
    bytes: bytes.length,
  };
}

type ResolvedTtsPlayback = {
  provider: TtsProvider;
  prefetch?: { blob: Blob; bytes: number };
};

async function resolveTtsPlaybackProvider(
  text: string,
  lang: string,
  provider: TtsProvider,
): Promise<ResolvedTtsPlayback> {
  if (provider !== "python_stdlib") {
    return { provider };
  }
  try {
    const fetched = await fetchTtsAudioBlob(text, lang, provider);
    return {
      provider,
      prefetch: { blob: fetched.blob, bytes: fetched.bytes },
    };
  } catch (primaryErr) {
    const message =
      primaryErr instanceof Error ? primaryErr.message : String(primaryErr);
    ttsTrace("google_tts", "fallback_to_rust_proxy", { message });
    return { provider: "browser_google" };
  }
}

async function prefetchGoogleTtsChunk(
  text: string,
  lang: string,
  requestedProvider: TtsProvider,
): Promise<PreparedGoogleTtsChunk> {
  const tl = normalizeGoogleTtsLang(lang);
  const resolved = await resolveTtsPlaybackProvider(text, tl, requestedProvider);
  if (resolved.prefetch) {
    return {
      blob: resolved.prefetch.blob,
      bytes: resolved.prefetch.bytes,
      provider: resolved.provider,
      textLen: text.length,
    };
  }
  const fetched = await fetchTtsAudioBlob(text, tl, resolved.provider);
  return {
    blob: fetched.blob,
    bytes: fetched.bytes,
    provider: fetched.provider,
    textLen: text.length,
  };
}

/** Download all audio chunks for a line (parallel per chunk). */
export async function prefetchGoogleTts(
  text: string,
  options: GoogleTtsPrefetchOptions,
): Promise<PreparedGoogleTts> {
  const chunks = chunkTextForGoogleTts(text);
  if (!chunks.length) {
    return { chunks: [] };
  }
  const provider = options.provider ?? "browser_google";
  const tl = normalizeGoogleTtsLang(options.lang);
  const prepared = await Promise.all(
    chunks.map((chunk) => prefetchGoogleTtsChunk(chunk, tl, provider)),
  );
  ttsTrace("google_tts", "prefetch_ready", {
    tl,
    text_len: text.length,
    chunk_count: prepared.length,
    bytes: prepared.reduce((sum, entry) => sum + entry.bytes, 0),
    provider,
  });
  return { chunks: prepared };
}

async function playPreparedGoogleTtsChunk(
  chunk: PreparedGoogleTtsChunk,
  options: GoogleTtsPlaybackOptions,
): Promise<void> {
  const rate = clampRate(options.rate);
  const volume =
    typeof options.volume === "number" && Number.isFinite(options.volume)
      ? Math.min(1, Math.max(0, options.volume))
      : 1;

  const objectUrl = URL.createObjectURL(chunk.blob);
  const audio = new Audio(objectUrl);
  audio.preload = "auto";
  options.onAudio?.(audio);
  if (typeof options.rate === "number" && Number.isFinite(options.rate)) {
    setPreservesPitch(audio, true);
    audio.playbackRate = rate;
  }
  if (typeof options.volume === "number" && Number.isFinite(options.volume)) {
    audio.volume = volume;
  }

  ttsTrace("google_tts", "play_start", {
    tl: normalizeGoogleTtsLang(options.lang),
    lang: normalizeGoogleTtsLang(options.lang),
    text_len: chunk.textLen,
    provider: chunk.provider,
    url_path: ttsProviderPath(chunk.provider),
    bytes: chunk.bytes,
    audio_url: "blob:",
    prefetch: true,
  });

  if (options.sinkId && "setSinkId" in audio) {
    try {
      await (
        audio as HTMLAudioElement & {
          setSinkId: (deviceId: string) => Promise<void>;
        }
      ).setSinkId(options.sinkId);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      ttsTrace("google_tts", "set_sink_error", { message, sink_id: options.sinkId });
      throw new Error(`Audio output device error: ${message}`);
    }
  }

  await waitForHtmlAudio(audio, objectUrl);
}

async function waitForHtmlAudio(
  audio: HTMLAudioElement,
  objectUrl: string,
): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    let settled = false;
    const finish = (ok: boolean, message: string, extra: Record<string, unknown> = {}) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeoutId);
      audio.onended = null;
      audio.onerror = null;
      audio.pause();
      audio.removeAttribute("src");
      audio.load();
      URL.revokeObjectURL(objectUrl);
      if (!ok) {
        ttsTrace("google_tts", "play_error", { message, ...extra });
        reject(new Error(message));
        return;
      }
      resolve();
    };

    const timeoutId = setTimeout(() => {
      finish(false, `Google TTS playback timeout after ${GOOGLE_TTS_PLAYBACK_TIMEOUT_MS}ms`, {
        stage: "timeout",
        ...mediaErrorFields(audio),
      });
    }, GOOGLE_TTS_PLAYBACK_TIMEOUT_MS);

    audio.onended = () => finish(true, "");
    audio.onerror = () => {
      finish(false, formatMediaError(audio), {
        stage: "media_element",
        ...mediaErrorFields(audio),
      });
    };

    void audio.play().catch((err: unknown) => {
      const message = err instanceof Error ? err.message : String(err);
      const name = err instanceof Error ? err.name : "Error";
      finish(false, `Google TTS play() rejected: ${name}: ${message}`, {
        stage: "play",
        play_error_name: name,
      });
    });
  });
}

/** Play audio that was already prefetched (no network wait at start). */
export async function playPreparedGoogleTts(
  prepared: PreparedGoogleTts,
  options: GoogleTtsPlaybackOptions,
): Promise<void> {
  for (const chunk of prepared.chunks) {
    await playPreparedGoogleTtsChunk(chunk, options);
  }
}

export async function playGoogleTts(
  text: string,
  options: GoogleTtsPlaybackOptions,
): Promise<void> {
  const prepared = await prefetchGoogleTts(text, options);
  await playPreparedGoogleTts(prepared, options);
}
