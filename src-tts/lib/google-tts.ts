import { ttsTrace } from "./tts-trace";
import type { TtsProvider } from "./types";

/** Google translate_tts practical per-request limit. */
export const GOOGLE_TTS_MAX_CHARS = 200;

/** Unicode scalar count — matches Rust `str.chars().count()` used by the TTS proxy. */
export function textCharCount(text: string): number {
  return Array.from(text).length;
}

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
  /** Raw MPEG audio bytes (single owner — no Blob duplicate). */
  data: Uint8Array;
  bytes: number;
  provider: TtsProvider;
  textLen: number;
};

export type PreparedGoogleTts = {
  chunks: (PreparedGoogleTtsChunk | undefined)[];
  /** Total chunks expected for this line. */
  expectedChunkCount: number;
  /** How many chunk slots are filled so far. */
  readyCount: number;
  /** Set when prefetch fails so waiters do not spin forever. */
  prefetchError?: string;
};

export type PrefetchProgressCallback = (
  prepared: PreparedGoogleTts,
  readyIndex: number,
) => void;

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

let ttsFetchWarmed = false;

/** Prime the local TTS proxy connection (TCP/TLS handshake) before first subtitle. */
export function warmupTtsFetch(lang = "en"): void {
  if (ttsFetchWarmed || typeof fetch === "undefined") return;
  ttsFetchWarmed = true;
  const url = buildTtsAudioUrl(".", lang, "browser_google");
  void fetch(url, { cache: "no-store", keepalive: true }).catch(() => {
    ttsFetchWarmed = false;
  });
  ttsTrace("google_tts", "warmup_started", { lang });
}

async function fetchTtsAudioBytes(
  text: string,
  lang: string,
  provider: TtsProvider,
): Promise<{ data: Uint8Array; provider: TtsProvider; bytes: number }> {
  const url = buildTtsAudioUrl(text, lang, provider);
  const response = await fetch(url, { cache: "no-store", keepalive: true });
  if (!response.ok) {
    throw new Error(
      `TTS HTTP ${response.status} from ${ttsProviderPath(provider)}`,
    );
  }
  const buffer = await response.arrayBuffer();
  const data = new Uint8Array(buffer);
  if (!looksLikeMpegAudio(data)) {
    throw new Error(
      `TTS response is not MPEG audio (${data.length} bytes from ${ttsProviderPath(provider)})`,
    );
  }
  return {
    data,
    provider,
    bytes: data.length,
  };
}

type ResolvedTtsPlayback = {
  provider: TtsProvider;
  prefetch?: { data: Uint8Array; bytes: number };
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
    const fetched = await fetchTtsAudioBytes(text, lang, provider);
    return {
      provider,
      prefetch: { data: fetched.data, bytes: fetched.bytes },
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
      data: resolved.prefetch.data,
      bytes: resolved.prefetch.bytes,
      provider: resolved.provider,
      textLen: text.length,
    };
  }
  const fetched = await fetchTtsAudioBytes(text, tl, resolved.provider);
  return {
    data: fetched.data,
    bytes: fetched.bytes,
    provider: fetched.provider,
    textLen: text.length,
  };
}

/** Wait until chunk `index` is available on a progressively filled prepared object. */
export async function waitForPreparedChunk(
  prepared: PreparedGoogleTts,
  index: number,
): Promise<PreparedGoogleTtsChunk> {
  if (prepared.chunks[index]) {
    return prepared.chunks[index]!;
  }
  while (!prepared.chunks[index]) {
    if (prepared.prefetchError) {
      throw new Error(prepared.prefetchError);
    }
    if (prepared.readyCount >= prepared.expectedChunkCount) {
      throw new Error(`TTS chunk ${index} missing after prefetch completed`);
    }
    await new Promise<void>((resolve) => setTimeout(resolve, 4));
  }
  return prepared.chunks[index]!;
}

/**
 * Download audio chunks with low time-to-first-byte:
 * fetch chunk 0 first, then remaining chunks in parallel.
 */
export async function prefetchGoogleTts(
  text: string,
  options: GoogleTtsPrefetchOptions,
  onProgress?: PrefetchProgressCallback,
): Promise<PreparedGoogleTts> {
  const textChunks = chunkTextForGoogleTts(text);
  if (!textChunks.length) {
    return { chunks: [], expectedChunkCount: 0, readyCount: 0 };
  }
  const provider = options.provider ?? "browser_google";
  const tl = normalizeGoogleTtsLang(options.lang);
  const prepared: PreparedGoogleTts = {
    chunks: new Array(textChunks.length),
    expectedChunkCount: textChunks.length,
    readyCount: 0,
  };

  const first = await prefetchGoogleTtsChunk(textChunks[0], tl, provider);
  prepared.chunks[0] = first;
  prepared.readyCount = 1;
  onProgress?.(prepared, 0);

  if (textChunks.length === 1) {
    ttsTrace("google_tts", "prefetch_ready", {
      tl,
      text_len: text.length,
      chunk_count: 1,
      bytes: first.bytes,
      provider,
      first_chunk_ms: 0,
    });
    return prepared;
  }

  await Promise.all(
    textChunks.slice(1).map(async (textChunk, offset) => {
      const index = offset + 1;
      const chunk = await prefetchGoogleTtsChunk(textChunk, tl, provider);
      prepared.chunks[index] = chunk;
      prepared.readyCount += 1;
      onProgress?.(prepared, index);
      return chunk;
    }),
  );

  ttsTrace("google_tts", "prefetch_ready", {
    tl,
    text_len: text.length,
    chunk_count: prepared.readyCount,
    expected_chunk_count: prepared.expectedChunkCount,
    bytes: prepared.chunks.reduce((sum, entry) => sum + (entry?.bytes ?? 0), 0),
    provider,
  });
  return prepared;
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

  const objectUrl = URL.createObjectURL(new Blob([chunk.data], { type: "audio/mpeg" }));
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
      ttsTrace("google_tts", "set_sink_fallback_default", { sink_id: options.sinkId });
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
  const total = prepared.expectedChunkCount || prepared.chunks.length;
  for (let index = 0; index < total; index += 1) {
    const chunk = await waitForPreparedChunk(prepared, index);
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
