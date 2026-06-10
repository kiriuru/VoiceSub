import type { TtsConfig, TtsProvider } from "./types";
import type { AudioPlayer, SpeechChannel } from "./audio-player";
import {
  channelBeginNext,
  channelClear,
  channelEnqueue,
  channelFinish,
  channelSnapshot,
} from "./tts-ipc";
import {
  normalizeGoogleTtsLang,
  prefetchGoogleTts,
  type PreparedGoogleTts,
} from "./google-tts";
import { ttsTrace, ttsTraceText } from "./tts-trace";

export type SpeechEngineEvent =
  | {
      type: "started";
      channel: SpeechChannel;
      id: string;
      text: string;
      lang: string;
      provider: TtsProvider;
    }
  | { type: "ended"; channel: SpeechChannel; id: string; lang: string }
  | {
      type: "error";
      channel: SpeechChannel;
      id: string;
      message: string;
      lang: string;
    };

type Listener = (event: SpeechEngineEvent) => void;

type SpeechJob = {
  id: string;
  text: string;
  lang: string;
  prepared?: PreparedGoogleTts;
  prefetchError?: string;
  prefetchPromise?: Promise<void>;
};

const PREFETCH_AHEAD_MAX = 2;

export class SpeechEngine {
  private readonly prefetchById = new Map<string, SpeechJob>();
  private speaking = false;
  private enabled = true;
  private config: TtsConfig;
  private readonly channel: SpeechChannel;
  private player: AudioPlayer;
  private listeners = new Set<Listener>();
  private currentId: string | null = null;
  private epoch = 0;

  constructor(channel: SpeechChannel, player: AudioPlayer, config: TtsConfig) {
    this.channel = channel;
    this.player = player;
    this.config = config;
  }

  getChannel(): SpeechChannel {
    return this.channel;
  }

  setPlayer(player: AudioPlayer) {
    this.player = player;
  }

  on(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  setConfig(config: TtsConfig) {
    this.config = config;
    const playback = this.playbackConfig();
    ttsTrace("engine", "config_updated", {
      channel: this.channel,
      enabled: config.enabled,
      playback_mode: config.playback_mode ?? "native",
      tts_provider: this.resolveProvider(config),
      speech_rate: playback.rate,
      speech_volume: playback.volume,
      prefetch_ahead_max: PREFETCH_AHEAD_MAX,
    });
  }

  private resolveProvider(config: TtsConfig): TtsProvider {
    return config.tts_provider === "python_stdlib" ? "python_stdlib" : "browser_google";
  }

  private playbackConfig(): {
    rate: number;
    volume: number;
    sinkId?: string;
  } {
    if (this.channel === "twitch" && this.config.twitch) {
      const twitch = this.config.twitch;
      const rate =
        typeof twitch.speech_rate === "number" && twitch.speech_rate > 0
          ? twitch.speech_rate
          : this.config.speech_rate;
      const volume =
        typeof twitch.speech_volume === "number" && twitch.speech_volume >= 0
          ? twitch.speech_volume
          : this.config.speech_volume;
      const sinkId = twitch.audio_output_device_id || undefined;
      return { rate, volume, sinkId };
    }
    return {
      rate: this.config.speech_rate,
      volume: this.config.speech_volume,
      sinkId: this.config.audio_output_device_id || undefined,
    };
  }

  setEnabled(enabled: boolean) {
    this.enabled = enabled;
    ttsTrace("engine", "enabled_changed", { channel: this.channel, enabled });
    if (!enabled) {
      void this.cancel();
    }
  }

  enqueue(id: string, text: string, lang = "en") {
    if (!text.trim()) return;
    const normalizedLang = normalizeGoogleTtsLang(lang);
    const job: SpeechJob = { id, text: text.trim(), lang: normalizedLang };
    this.prefetchById.set(id, job);
    void channelEnqueue(this.channel, id, job.text, normalizedLang)
      .then((queueLen) => {
        ttsTraceText("engine", "enqueue", job.text, {
          channel: this.channel,
          id,
          lang: normalizedLang,
          queue_len: queueLen,
        });
        void this.schedulePrefetchFromRust();
        void this.pump();
      })
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : String(err);
        ttsTrace("engine", "enqueue_error", { channel: this.channel, id, message });
      });
  }

  clear() {
    const dropped = this.prefetchById.size;
    this.prefetchById.clear();
    void channelClear(this.channel).catch(() => {});
    void this.player.stop();
    this.cancelCurrent();
    ttsTrace("engine", "clear", { channel: this.channel, dropped });
  }

  private emit(event: SpeechEngineEvent) {
    for (const listener of this.listeners) {
      listener(event);
    }
  }

  private bumpEpoch() {
    this.epoch += 1;
  }

  private cancelCurrent() {
    this.bumpEpoch();
    void this.player.stop();
    this.speaking = false;
    this.currentId = null;
  }

  private async cancel() {
    this.prefetchById.clear();
    await channelClear(this.channel).catch(() => {});
    this.cancelCurrent();
  }

  private async schedulePrefetchFromRust() {
    const epoch = this.epoch;
    let snapshot: Awaited<ReturnType<typeof channelSnapshot>> = [];
    try {
      snapshot = await channelSnapshot(this.channel);
    } catch {
      return;
    }
    let inFlight = [...this.prefetchById.values()].filter(
      (job) => job.prefetchPromise && !job.prepared && !job.prefetchError,
    ).length;
    for (const entry of snapshot) {
      if (inFlight >= PREFETCH_AHEAD_MAX) break;
      const job = this.prefetchById.get(entry.id) ?? {
        id: entry.id,
        text: entry.text,
        lang: entry.lang,
      };
      this.prefetchById.set(entry.id, job);
      if (job.prepared || job.prefetchError || job.prefetchPromise) continue;
      inFlight += 1;
      job.prefetchPromise = this.runPrefetch(job, epoch);
    }
  }

  private async runPrefetch(job: SpeechJob, epoch: number): Promise<void> {
    try {
      const prepared = await prefetchGoogleTts(job.text, {
        lang: job.lang,
        provider: this.resolveProvider(this.config),
      });
      if (epoch !== this.epoch) return;
      job.prepared = prepared;
      ttsTrace("engine", "prefetch_ready", {
        channel: this.channel,
        id: job.id,
        chunk_count: prepared.chunks.length,
      });
      void this.pump();
    } catch (err) {
      if (epoch !== this.epoch) return;
      job.prefetchError = err instanceof Error ? err.message : String(err);
      ttsTrace("engine", "prefetch_error", {
        channel: this.channel,
        id: job.id,
        message: job.prefetchError,
      });
      void this.pump();
    }
  }

  private async ensureJobReady(job: SpeechJob, epoch: number): Promise<void> {
    if (job.prepared || job.prefetchError) return;
    if (!job.prefetchPromise) {
      job.prefetchPromise = this.runPrefetch(job, epoch);
    }
    await job.prefetchPromise;
  }

  private async finishRustItem(itemId: string) {
    try {
      await channelFinish(this.channel, itemId);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      ttsTrace("engine", "finish_error", { channel: this.channel, id: itemId, message });
    }
    this.prefetchById.delete(itemId);
  }

  private async pump() {
    if (!this.enabled || this.speaking) return;

    const epoch = this.epoch;
    let item: Awaited<ReturnType<typeof channelBeginNext>> = null;
    try {
      item = await channelBeginNext(this.channel);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      ttsTrace("engine", "begin_next_error", { channel: this.channel, message });
      return;
    }

    if (!item || epoch !== this.epoch || !this.enabled) return;

    const job =
      this.prefetchById.get(item.id) ??
      ({
        id: item.id,
        text: item.text,
        lang: item.lang,
      } satisfies SpeechJob);
    this.prefetchById.set(item.id, job);

    try {
      await this.ensureJobReady(job, epoch);
    } catch {
      // handled below
    }

    if (epoch !== this.epoch || !this.enabled) {
      await this.finishRustItem(item.id);
      return;
    }

    if (job.prefetchError) {
      const message = job.prefetchError;
      ttsTrace("engine", "google_tts_error", {
        channel: this.channel,
        id: job.id,
        message,
        stage: "prefetch",
      });
      this.emit({
        type: "error",
        channel: this.channel,
        id: job.id,
        message,
        lang: job.lang,
      });
      await this.finishRustItem(item.id);
      void this.schedulePrefetchFromRust();
      void this.pump();
      return;
    }

    if (!job.prepared?.chunks.length) {
      await this.finishRustItem(item.id);
      void this.pump();
      return;
    }

    this.speaking = true;
    this.currentId = job.id;
    const playback = this.playbackConfig();

    ttsTraceText("engine", "speak_start", job.text, {
      channel: this.channel,
      id: job.id,
      lang: job.lang,
      tts_provider: this.resolveProvider(this.config),
      rate: playback.rate,
      volume: playback.volume,
      prefetched: true,
    });
    this.emit({
      type: "started",
      channel: this.channel,
      id: job.id,
      text: job.text,
      lang: job.lang,
      provider: this.resolveProvider(this.config),
    });

    try {
      await this.player.playPrepared(job.prepared, {
        lang: job.lang,
        provider: this.resolveProvider(this.config),
        rate: playback.rate,
        volume: playback.volume,
        sinkId: playback.sinkId,
        itemId: job.id,
      });
      if (epoch !== this.epoch) return;
      ttsTrace("engine", "speak_end", {
        channel: this.channel,
        id: job.id,
        tts_provider: this.resolveProvider(this.config),
      });
      this.emit({
        type: "ended",
        channel: this.channel,
        id: job.id,
        lang: job.lang,
      });
    } catch (err) {
      if (epoch !== this.epoch) return;
      const message = err instanceof Error ? err.message : String(err);
      ttsTrace("engine", "google_tts_error", {
        channel: this.channel,
        id: job.id,
        message,
      });
      this.emit({
        type: "error",
        channel: this.channel,
        id: job.id,
        message,
        lang: job.lang,
      });
    } finally {
      this.speaking = false;
      this.currentId = null;
      await this.finishRustItem(job.id);
      void this.schedulePrefetchFromRust();
      void this.pump();
    }
  }
}
