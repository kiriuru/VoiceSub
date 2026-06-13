import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  GOOGLE_TTS_PLAYBACK_TIMEOUT_MS,
  waitForPreparedChunk,
  type GoogleTtsPlaybackOptions,
  type PreparedGoogleTts,
  type PreparedGoogleTtsChunk,
} from "./google-tts";
import type { TtsPlaybackMode } from "./types";
import { ttsTrace } from "./tts-trace";

export type SpeechChannel = "speech" | "twitch";

export interface AudioPlayer {
  playPrepared(
    prepared: PreparedGoogleTts,
    options: GoogleTtsPlaybackOptions & { itemId: string },
  ): Promise<void>;
  stop(): Promise<void>;
}

type PlaybackFinishedPayload = {
  channel: string;
  item_id: string;
  ok: boolean;
  error?: string | null;
};

type WaitEntry = {
  resolve: () => void;
  reject: (error: Error) => void;
};

const pendingWaits = new Map<string, WaitEntry>();
let listenerReady: Promise<UnlistenFn> | null = null;

function waitKey(channel: string, itemId: string): string {
  return `${channel}:${itemId}`;
}

function ensurePlaybackListener(): Promise<UnlistenFn> {
  if (!listenerReady) {
    listenerReady = listen<PlaybackFinishedPayload>("playback-finished", (event) => {
      const payload = event.payload;
      const key = waitKey(payload.channel, payload.item_id);
      const wait = pendingWaits.get(key);
      if (!wait) return;
      pendingWaits.delete(key);
      if (payload.ok) {
        wait.resolve();
        return;
      }
      wait.reject(new Error(payload.error || "native playback failed"));
    });
  }
  return listenerReady;
}

function rejectPendingWaits(channel: SpeechChannel, message: string) {
  const prefix = `${channel}:`;
  for (const [key, wait] of pendingWaits) {
    if (!key.startsWith(prefix)) continue;
    pendingWaits.delete(key);
    wait.reject(new Error(message));
  }
}

export function isNativePlaybackMode(
  playbackMode: string | undefined | null,
): boolean {
  return String(playbackMode || "").trim().toLowerCase() === "native";
}

export function isSonicPlaybackMode(
  playbackMode: string | undefined | null,
): boolean {
  const mode = String(playbackMode || "").trim().toLowerCase();
  return mode === "sonic" || mode === "browser";
}

export class NativeAudioPlayer implements AudioPlayer {
  private stopped = false;

  constructor(
    private readonly channel: SpeechChannel,
    private readonly playbackMode: TtsPlaybackMode = "native",
  ) {}

  async playPrepared(
    prepared: PreparedGoogleTts,
    options: GoogleTtsPlaybackOptions & { itemId: string },
  ): Promise<void> {
    this.stopped = false;
    await ensurePlaybackListener();
    const volume = options.volume ?? 1;
    const rate = isNativePlaybackMode(this.playbackMode) ? 1 : (options.rate ?? 1);
    const total = prepared.expectedChunkCount || prepared.chunks.length;
    let nextChunkPromise: Promise<PreparedGoogleTtsChunk> | null = null;
    for (let chunkIndex = 0; chunkIndex < total; chunkIndex += 1) {
      if (this.stopped) {
        throw new Error("playback stopped");
      }
      const chunkItemId = `${options.itemId}#${chunkIndex}`;
      const chunk = nextChunkPromise
        ? await nextChunkPromise
        : await waitForPreparedChunk(prepared, chunkIndex);
      nextChunkPromise = null;
      if (chunkIndex + 1 < total) {
        nextChunkPromise = waitForPreparedChunk(prepared, chunkIndex + 1);
      }
      const bytes = chunk.data;
      ttsTrace("native_audio", "play_enqueue", {
        channel: this.channel,
        item_id: chunkItemId,
        bytes: bytes.length,
        volume,
        rate,
        playback_mode: this.playbackMode,
        chunk_index: chunkIndex,
        chunk_total: total,
      });
      await new Promise<void>((resolve, reject) => {
        if (this.stopped) {
          reject(new Error("playback stopped"));
          return;
        }
        const key = waitKey(this.channel, chunkItemId);
        let settled = false;
        const finish = (ok: boolean, message: string) => {
          if (settled) return;
          settled = true;
          clearTimeout(timeoutId);
          pendingWaits.delete(key);
          if (ok) {
            resolve();
            return;
          }
          reject(new Error(message));
        };
        const timeoutId = setTimeout(() => {
          finish(
            false,
            `Native TTS playback timeout after ${GOOGLE_TTS_PLAYBACK_TIMEOUT_MS}ms`,
          );
        }, GOOGLE_TTS_PLAYBACK_TIMEOUT_MS);
        pendingWaits.set(key, {
          resolve: () => finish(true, ""),
          reject: (error: Error) => finish(false, error.message),
        });
        invoke("tts_play_audio", {
          channel: this.channel,
          itemId: chunkItemId,
          audioBytes: bytes,
          volume,
          rate,
        })
          .then(() => {
            prepared.chunks[chunkIndex] = undefined;
          })
          .catch((err: unknown) => {
            finish(false, err instanceof Error ? err.message : String(err));
          });
      });
    }
  }

  async stop(): Promise<void> {
    this.stopped = true;
    rejectPendingWaits(this.channel, "playback stopped");
    try {
      await invoke("tts_stop_channel", { channel: this.channel });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      ttsTrace("native_audio", "stop_error", { channel: this.channel, message });
    }
  }
}

export function createAudioPlayer(
  channel: SpeechChannel,
  playbackMode: TtsPlaybackMode | string | undefined | null,
): AudioPlayer {
  const mode = isNativePlaybackMode(playbackMode)
    ? "native"
    : isSonicPlaybackMode(playbackMode)
      ? "sonic"
      : "native";
  return new NativeAudioPlayer(channel, mode);
}
