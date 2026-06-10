import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  playPreparedGoogleTts,
  type GoogleTtsPlaybackOptions,
  type PreparedGoogleTts,
} from "./google-tts";
import { ttsTrace } from "./tts-trace";

export type SpeechChannel = "speech" | "twitch";

export interface AudioPlayer {
  playPrepared(
    prepared: PreparedGoogleTts,
    options: GoogleTtsPlaybackOptions & { itemId: string },
  ): Promise<void>;
  stop(): Promise<void>;
}

export class HtmlAudioPlayer implements AudioPlayer {
  private readonly activeAudios = new Set<HTMLAudioElement>();

  async playPrepared(
    prepared: PreparedGoogleTts,
    options: GoogleTtsPlaybackOptions & { itemId: string },
  ): Promise<void> {
    await playPreparedGoogleTts(prepared, {
      ...options,
      onAudio: (audio) => {
        this.activeAudios.add(audio);
        audio.addEventListener(
          "ended",
          () => {
            this.activeAudios.delete(audio);
          },
          { once: true },
        );
        options.onAudio?.(audio);
      },
    });
  }

  async stop(): Promise<void> {
    for (const audio of this.activeAudios) {
      audio.pause();
      audio.removeAttribute("src");
      audio.load();
    }
    this.activeAudios.clear();
  }
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

export class NativeAudioPlayer implements AudioPlayer {
  private stopped = false;

  constructor(private readonly channel: SpeechChannel) {}

  async playPrepared(
    prepared: PreparedGoogleTts,
    options: GoogleTtsPlaybackOptions & { itemId: string },
  ): Promise<void> {
    this.stopped = false;
    await ensurePlaybackListener();
    const volume = options.volume ?? 1;
    const rate = options.rate ?? 1;
    let chunkIndex = 0;
    for (const chunk of prepared.chunks) {
      if (this.stopped) {
        throw new Error("playback stopped");
      }
      const chunkItemId = `${options.itemId}#${chunkIndex}`;
      chunkIndex += 1;
      const bytes = new Uint8Array(await chunk.blob.arrayBuffer());
      ttsTrace("native_audio", "play_enqueue", {
        channel: this.channel,
        item_id: chunkItemId,
        bytes: bytes.length,
        volume,
        rate,
      });
      await new Promise<void>((resolve, reject) => {
        if (this.stopped) {
          reject(new Error("playback stopped"));
          return;
        }
        const key = waitKey(this.channel, chunkItemId);
        pendingWaits.set(key, { resolve, reject });
        invoke("tts_play_audio", {
          channel: this.channel,
          itemId: chunkItemId,
          audioBytes: bytes,
          volume,
          rate,
        }).catch((err: unknown) => {
          pendingWaits.delete(key);
          reject(err instanceof Error ? err : new Error(String(err)));
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

export function isNativePlaybackMode(
  playbackMode: string | undefined | null,
): boolean {
  return String(playbackMode || "").trim().toLowerCase() === "native";
}

export function createAudioPlayer(
  channel: SpeechChannel,
  playbackMode: string | undefined | null,
): AudioPlayer {
  if (isNativePlaybackMode(playbackMode)) {
    return new NativeAudioPlayer(channel);
  }
  return new HtmlAudioPlayer();
}
