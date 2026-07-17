import { apiFetch } from "./loopback-api-client";

export type DepDownloadKind = "ort_cpu" | "ort_gpu" | "cuda_redist";
export type ModelFamily = "parakeet_tdt";
export type ModelVariant = string;
export type TransferPhase = "idle" | "downloading" | "extracting" | "finalizing";

export interface TransferProgress {
  active: boolean;
  phase: TransferPhase;
  label: string;
  target: string;
  receivedBytes: number;
  totalBytes: number | null;
  speedBps: number;
  percent: number | null;
  error: string | null;
  cancelled: boolean;
}

function readTransfer(raw: Record<string, unknown> | undefined): TransferProgress {
  const t = raw ?? {};
  const phaseRaw = String(t.phase ?? "idle");
  const phase: TransferPhase =
    phaseRaw === "downloading" ||
    phaseRaw === "extracting" ||
    phaseRaw === "finalizing"
      ? phaseRaw
      : "idle";
  return {
    active: Boolean(t.active),
    phase,
    label: String(t.label ?? ""),
    target: String(t.target ?? ""),
    receivedBytes: Number(t.receivedBytes ?? t.received_bytes ?? 0),
    totalBytes:
      t.totalBytes != null || t.total_bytes != null
        ? Number(t.totalBytes ?? t.total_bytes)
        : null,
    speedBps: Number(t.speedBps ?? t.speed_bps ?? 0),
    percent: t.percent != null ? Number(t.percent) : null,
    error: (t.error ?? null) as string | null,
    cancelled: Boolean(t.cancelled),
  };
}

export function formatTransferBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit >= 2 ? 1 : 0;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

export function formatTransferSpeed(bps: number): string {
  return `${formatTransferBytes(bps)}/s`;
}

export function transferEffectiveTotalBytes(item: TransferProgress): number | null {
  const total = item.totalBytes;
  if (total != null && total > 0) {
    return Math.max(total, item.receivedBytes);
  }
  return null;
}

export function transferDisplayPercent(item: TransferProgress): number | null {
  const total = transferEffectiveTotalBytes(item);
  if (total != null) {
    return Math.min(100, Math.max(0, (item.receivedBytes / total) * 100));
  }
  if (item.percent != null && Number.isFinite(item.percent)) {
    return Math.min(100, Math.max(0, item.percent));
  }
  return null;
}

export function formatTransferPercent(value: number): string {
  if (value >= 100) return "100%";
  if (value >= 10) return `${Math.round(value)}%`;
  return `${value.toFixed(1)}%`;
}

export interface ModelCatalogEntry {
  family?: ModelFamily;
  variant: ModelVariant;
  installed: boolean;
  sizeMb: number;
  active: boolean;
  sourceAuthor: string;
}

export interface DllGroupStatus {
  ok: boolean;
  missing: string[];
  found: { name: string; path: string }[];
  downloadMb: number;
}

export interface CudaToolkitStatus {
  ok: boolean;
  version?: string | null;
  message?: string | null;
}

export interface LocalAsrEnvCheck {
  vcruntime: DllGroupStatus;
  ortCpu: DllGroupStatus;
  ortGpu: DllGroupStatus;
  cudaRedist: DllGroupStatus;
  cudaToolkit: CudaToolkitStatus;
  cpuDepsReady: boolean;
  cudaDepsReady: boolean;
}

export interface LocalAsrSetupChecklist {
  depsReady: boolean;
  modelInstalled: boolean;
  micTestPassed: boolean;
  parakeetFinalReceived: boolean;
  setupComplete: boolean;
}

/** P0 decode phase timing — VoiceSub stages vs Parakeet black-box. */
export interface DecodeTimingBreakdown {
  audioSamples: number;
  audioMs: number;
  prepareUs: number;
  preprocessUs: number;
  parakeetTranscribeUs: number;
  totalUs: number;
  outsideUs: number;
  outsidePct: number;
  parakeetPct: number;
}

export interface LocalAsrModuleStatus {
  phase: string;
  ready: boolean;
  cudaReady: boolean;
  depsReady: boolean;
  executionProvider: string;
  activeExecutionProvider: string;
  message: string;
  lastError?: string | null;
  env: LocalAsrEnvCheck;
  modelInstalled: boolean;
  modelLoaded: boolean;
  modelLoadMs: number | null;
  probeCpuOk: boolean | null;
  probeCudaOk: boolean | null;
  ortProfilingActive: boolean;
  ortProfilingDecodeCount: number;
  ortProfilingMaxDecodes: number;
  ortProfilingStoppedBudget: boolean;
  lastOrtProfilePath: string | null;
  lastDecodeTiming: DecodeTimingBreakdown | null;
  activeModelFamily: ModelFamily;
  activeModelVariant: ModelVariant;
  models: ModelCatalogEntry[];
  setup: LocalAsrSetupChecklist;
}

export type TestBenchPhase = "idle" | "listening" | "done" | "error";

export interface TestBenchSnapshot {
  running: boolean;
  phase: TestBenchPhase;
  executionProvider: string;
  modelLabel: string;
  durationMs: number;
  deviceId: string;
  deviceLabel: string;
  sampleCount: number;
  sampleRate: number;
  partialTranscript: string;
  transcript: string;
  finalizedSegments: number;
  isSpeech: boolean;
  decodeCount: number;
  captureMs: number;
  lastDecodeMs: number;
  lastDecodeTiming: DecodeTimingBreakdown | null;
  error: string | null;
}

export interface LocalAsrRealtimeConfig {
  latencyPreset: string;
  streamingDecode: boolean;
  partialEmitMode: "word_growth" | "char_delta";
  partialMinNewWords: number;
  partialMinDeltaChars: number;
  partialCoalescingMs: number;
  decodeIntervalMs?: number | null;
  windowMs?: number | null;
  segmentEnqueueDeltaMs?: number | null;
  firstPartialMinSpeechMs?: number | null;
}

export interface LocalAsrVadConfig {
  enabled: boolean;
  vadMode: number;
  energyGateEnabled: boolean;
  minRmsForRecognition: number;
  minVoicedRatio: number;
  speechAttackFrames: number;
  speechPrerollFrames: number;
  partialEmitIntervalMs?: number | null;
  speechThreshold: number;
  minSpeechMs: number;
  minSilenceMs: number;
  silenceHoldMs: number;
  speechPadMs: number;
  maxSegmentMs: number;
}

const VS_VAD_DEFAULTS: LocalAsrVadConfig = {
  enabled: true,
  vadMode: 2,
  energyGateEnabled: false,
  minRmsForRecognition: 0.0018,
  minVoicedRatio: 0,
  speechAttackFrames: 2,
  speechPrerollFrames: 5,
  partialEmitIntervalMs: null,
  speechThreshold: 0.015,
  minSpeechMs: 180,
  minSilenceMs: 400,
  silenceHoldMs: 180,
  speechPadMs: 0,
  maxSegmentMs: 5500,
};

const LATENCY_PRESET_VAD: Record<"low" | "balanced" | "quality", LocalAsrVadConfig> = {
  low: {
    ...VS_VAD_DEFAULTS,
    minSpeechMs: 180,
    minSilenceMs: 220,
    silenceHoldMs: 120,
  },
  balanced: {
    ...VS_VAD_DEFAULTS,
    minSpeechMs: 180,
    minSilenceMs: 400,
    silenceHoldMs: 180,
  },
  quality: {
    ...VS_VAD_DEFAULTS,
    minSpeechMs: 260,
    minSilenceMs: 520,
    silenceHoldMs: 260,
  },
};

export function normalizeLatencyPreset(value: string): "low" | "balanced" | "quality" {
  const raw = String(value || "balanced").trim().toLowerCase();
  if (raw === "quality" || raw === "high") return "quality";
  if (raw === "low" || raw === "fast" || raw === "ultra_low_latency" || raw === "ultra_low") {
    return "low";
  }
  return "balanced";
}

/** Apply SST preset timings to module config (VAD + partial coalescing). */
export function applyLatencyPreset(config: LocalAsrConfig, preset: string): LocalAsrConfig {
  const key = normalizeLatencyPreset(preset);
  return {
    ...config,
    realtime: {
      ...config.realtime,
      latencyPreset: key,
      streamingDecode: true,
      partialEmitMode: "word_growth",
      partialMinNewWords: 1,
      partialMinDeltaChars: key === "quality" ? 1 : 0,
      partialCoalescingMs: key === "quality" ? 80 : 0,
      decodeIntervalMs: null,
      windowMs: null,
      segmentEnqueueDeltaMs: null,
      firstPartialMinSpeechMs: null,
    },
    vad: {
      ...config.vad,
      ...LATENCY_PRESET_VAD[key],
    },
  };
}

export function defaultLocalAsrModuleConfig(): LocalAsrConfig {
  return applyLatencyPreset(
    {
      model: {
        family: "parakeet_tdt",
        variant: "int8",
        targetLang: "auto",
      },
      inference: {
        executionProvider: "cpu",
        graphOptimizationLevel: 1,
        intraOpThreads: 4,
        interOpThreads: 1,
        parallelExecution: false,
        enableMemoryPattern: true,
        ortProfiling: false,
        ortProfilingMaxDecodes: 3,
        keepModelLoaded: false,
        cudaFallbackToCpu: true,
      },
      realtime: {
        latencyPreset: "balanced",
        streamingDecode: true,
        partialEmitMode: "word_growth",
        partialMinNewWords: 1,
        partialMinDeltaChars: 0,
        partialCoalescingMs: 0,
        decodeIntervalMs: null,
        windowMs: null,
        segmentEnqueueDeltaMs: null,
        firstPartialMinSpeechMs: null,
      },
      vad: { ...VS_VAD_DEFAULTS },
      recognition: {
        inputGain: 1,
        preemphasisEnabled: false,
        preemphasisCoeff: 0.97,
        noiseGateEnabled: false,
        noiseGateThreshold: 0.008,
        hallucinationFilterEnabled: true,
        hallucinationMinChars: 0,
        hallucinationCooldownMs: 500,
      },
      microphone: {
        deviceId: "",
        sampleRate: 16000,
      },
    },
    "balanced",
  );
}

export interface LocalAsrRecognitionConfig {
  inputGain: number;
  preemphasisEnabled: boolean;
  preemphasisCoeff: number;
  noiseGateEnabled: boolean;
  noiseGateThreshold: number;
  hallucinationFilterEnabled: boolean;
  hallucinationMinChars: number;
  hallucinationCooldownMs: number;
}

export interface ProbeResult {
  provider: string;
  ok: boolean;
  loadMs: number;
  message: string;
  fallbackProvider: string | null;
}

export interface LoadResult {
  loaded: boolean;
  loadMs: number;
  activeExecutionProvider: string;
  message: string;
}

export interface LocalAsrConfig {
  model?: {
    family?: ModelFamily;
    variant?: ModelVariant;
    /** Legacy config field (ignored). Kept for sidecar config compatibility. */
    targetLang?: string;
    path?: string;
    manifestSha256?: string;
  };
  inference: {
    executionProvider: string;
    graphOptimizationLevel: number;
    intraOpThreads: number;
    interOpThreads: number;
    parallelExecution: boolean;
    enableMemoryPattern: boolean;
    ortProfiling: boolean;
    ortProfilingMaxDecodes: number;
    keepModelLoaded: boolean;
    cudaFallbackToCpu: boolean;
  };
  realtime: LocalAsrRealtimeConfig;
  vad: LocalAsrVadConfig;
  recognition: LocalAsrRecognitionConfig;
  microphone: {
    deviceId: string;
    sampleRate?: number;
  };
}

export interface InputDeviceInfo {
  id: string;
  label: string;
  isDefault: boolean;
}

function readOptionalBool(raw: unknown): boolean | null {
  if (raw === null || raw === undefined) return null;
  return Boolean(raw);
}

function readDecodeTiming(raw: unknown): DecodeTimingBreakdown | null {
  if (!raw || typeof raw !== "object") return null;
  const t = raw as Record<string, unknown>;
  return {
    audioSamples: Number(t.audioSamples ?? t.audio_samples ?? 0),
    audioMs: Number(t.audioMs ?? t.audio_ms ?? 0),
    prepareUs: Number(t.prepareUs ?? t.prepare_us ?? 0),
    preprocessUs: Number(t.preprocessUs ?? t.preprocess_us ?? 0),
    parakeetTranscribeUs: Number(t.parakeetTranscribeUs ?? t.parakeet_transcribe_us ?? 0),
    totalUs: Number(t.totalUs ?? t.total_us ?? 0),
    outsideUs: Number(t.outsideUs ?? t.outside_us ?? 0),
    outsidePct: Number(t.outsidePct ?? t.outside_pct ?? 0),
    parakeetPct: Number(t.parakeetPct ?? t.parakeet_pct ?? 0),
  };
}

function readTestBench(raw: Record<string, unknown> | undefined): TestBenchSnapshot {
  const t = raw ?? {};
  const phaseRaw = String(t.phase ?? "idle");
  const phase: TestBenchPhase =
    phaseRaw === "listening" ||
    phaseRaw === "done" ||
    phaseRaw === "error"
      ? phaseRaw
      : phaseRaw === "recording" || phaseRaw === "transcribing"
        ? "listening"
        : "idle";
  return {
    running: Boolean(t.running),
    phase,
    executionProvider: String(t.executionProvider ?? t.execution_provider ?? ""),
    modelLabel: String(t.modelLabel ?? t.model_label ?? ""),
    durationMs: Number(t.durationMs ?? t.duration_ms ?? 0),
    deviceId: String(t.deviceId ?? t.device_id ?? ""),
    deviceLabel: String(t.deviceLabel ?? t.device_label ?? ""),
    sampleCount: Number(t.sampleCount ?? t.sample_count ?? 0),
    sampleRate: Number(t.sampleRate ?? t.sample_rate ?? 16000),
    partialTranscript: String(t.partialTranscript ?? t.partial_transcript ?? ""),
    transcript: String(t.transcript ?? ""),
    finalizedSegments: Number(t.finalizedSegments ?? t.finalized_segments ?? 0),
    isSpeech: Boolean(t.isSpeech ?? t.is_speech),
    decodeCount: Number(t.decodeCount ?? t.decode_count ?? 0),
    captureMs: Number(t.captureMs ?? t.capture_ms ?? 0),
    lastDecodeMs: Number(t.lastDecodeMs ?? t.last_decode_ms ?? 0),
    lastDecodeTiming: readDecodeTiming(t.lastDecodeTiming ?? t.last_decode_timing),
    error: (t.error ?? null) as string | null,
  };
}

function readRealtime(raw: Record<string, unknown> | undefined): LocalAsrRealtimeConfig {
  const rt = raw ?? {};
  const mode = String(rt.partialEmitMode ?? rt.partial_emit_mode ?? "word_growth");
  return {
    latencyPreset: normalizeLatencyPreset(
      String(rt.latencyPreset ?? rt.latency_preset ?? "balanced"),
    ),
    streamingDecode: Boolean(rt.streamingDecode ?? rt.streaming_decode ?? true),
    partialEmitMode: mode === "char_delta" ? "char_delta" : "word_growth",
    partialMinNewWords: Number(rt.partialMinNewWords ?? rt.partial_min_new_words ?? 1),
    partialMinDeltaChars: Number(rt.partialMinDeltaChars ?? rt.partial_min_delta_chars ?? 0),
    partialCoalescingMs: Number(rt.partialCoalescingMs ?? rt.partial_coalescing_ms ?? 0),
    decodeIntervalMs:
      rt.decodeIntervalMs != null || rt.decode_interval_ms != null
        ? Number(rt.decodeIntervalMs ?? rt.decode_interval_ms)
        : null,
    windowMs:
      rt.windowMs != null || rt.window_ms != null
        ? Number(rt.windowMs ?? rt.window_ms)
        : null,
    segmentEnqueueDeltaMs:
      rt.segmentEnqueueDeltaMs != null || rt.segment_enqueue_delta_ms != null
        ? Number(rt.segmentEnqueueDeltaMs ?? rt.segment_enqueue_delta_ms)
        : null,
    firstPartialMinSpeechMs:
      rt.firstPartialMinSpeechMs != null || rt.first_partial_min_speech_ms != null
        ? Number(rt.firstPartialMinSpeechMs ?? rt.first_partial_min_speech_ms)
        : null,
  };
}

function readVad(raw: Record<string, unknown> | undefined): LocalAsrVadConfig {
  const vad = raw ?? {};
  const minRms = Number(
    vad.minRmsForRecognition ?? vad.min_rms_for_recognition ?? VS_VAD_DEFAULTS.minRmsForRecognition,
  );
  return {
    enabled: Boolean(vad.enabled ?? true),
    vadMode: Number(vad.vadMode ?? vad.vad_mode ?? VS_VAD_DEFAULTS.vadMode),
    energyGateEnabled: Boolean(
      vad.energyGateEnabled ?? vad.energy_gate_enabled ?? VS_VAD_DEFAULTS.energyGateEnabled,
    ),
    minRmsForRecognition: minRms,
    minVoicedRatio: Number(vad.minVoicedRatio ?? vad.min_voiced_ratio ?? VS_VAD_DEFAULTS.minVoicedRatio),
    speechAttackFrames: Number(
      vad.speechAttackFrames ?? vad.speech_attack_frames ?? VS_VAD_DEFAULTS.speechAttackFrames,
    ),
    speechPrerollFrames: Number(
      vad.speechPrerollFrames ?? vad.speech_preroll_frames ?? VS_VAD_DEFAULTS.speechPrerollFrames,
    ),
    partialEmitIntervalMs:
      vad.partialEmitIntervalMs != null || vad.partial_emit_interval_ms != null
        ? Number(vad.partialEmitIntervalMs ?? vad.partial_emit_interval_ms)
        : null,
    speechThreshold: Number(vad.speechThreshold ?? vad.speech_threshold ?? minRms),
    minSpeechMs: Number(vad.minSpeechMs ?? vad.min_speech_ms ?? VS_VAD_DEFAULTS.minSpeechMs),
    minSilenceMs: Number(vad.minSilenceMs ?? vad.min_silence_ms ?? VS_VAD_DEFAULTS.minSilenceMs),
    silenceHoldMs: Number(vad.silenceHoldMs ?? vad.silence_hold_ms ?? VS_VAD_DEFAULTS.silenceHoldMs),
    speechPadMs: Number(vad.speechPadMs ?? vad.speech_pad_ms ?? VS_VAD_DEFAULTS.speechPadMs),
    maxSegmentMs: Number(vad.maxSegmentMs ?? vad.max_segment_ms ?? VS_VAD_DEFAULTS.maxSegmentMs),
  };
}

function readRecognition(raw: Record<string, unknown> | undefined): LocalAsrRecognitionConfig {
  const rec = raw ?? {};
  return {
    inputGain: Number(rec.inputGain ?? rec.input_gain ?? 1),
    preemphasisEnabled: Boolean(rec.preemphasisEnabled ?? rec.preemphasis_enabled ?? false),
    preemphasisCoeff: Number(rec.preemphasisCoeff ?? rec.preemphasis_coeff ?? 0.97),
    noiseGateEnabled: Boolean(rec.noiseGateEnabled ?? rec.noise_gate_enabled ?? false),
    noiseGateThreshold: Number(rec.noiseGateThreshold ?? rec.noise_gate_threshold ?? 0.008),
    hallucinationFilterEnabled: Boolean(
      rec.hallucinationFilterEnabled ?? rec.hallucination_filter_enabled ?? true,
    ),
    hallucinationMinChars: Number(rec.hallucinationMinChars ?? rec.hallucination_min_chars ?? 0),
    hallucinationCooldownMs: Number(
      rec.hallucinationCooldownMs ?? rec.hallucination_cooldown_ms ?? 500,
    ),
  };
}

function normalizeConfig(raw: Record<string, unknown>): LocalAsrConfig {
  const inf = (raw.inference ?? {}) as Record<string, unknown>;
  const mic = (raw.microphone ?? {}) as Record<string, unknown>;
  const model = (raw.model ?? {}) as Record<string, unknown>;
  const family = parseModelFamily(String(model.family ?? "parakeet_tdt"));
  return {
    model: {
      family,
      variant: String(model.variant ?? defaultVariantForFamily(family)),
      targetLang: String(model.targetLang ?? model.target_lang ?? "auto").trim() || "auto",
      path: String(model.path ?? ""),
      manifestSha256: String(model.manifestSha256 ?? model.manifest_sha256 ?? ""),
    },
    inference: {
      executionProvider: String(inf.executionProvider ?? inf.execution_provider ?? "cpu"),
      graphOptimizationLevel: clampInt(
        Number(inf.graphOptimizationLevel ?? inf.graph_optimization_level ?? 1),
        0,
        3,
      ),
      intraOpThreads: clampInt(Number(inf.intraOpThreads ?? inf.intra_op_threads ?? 4), 1, 64),
      interOpThreads: clampInt(Number(inf.interOpThreads ?? inf.inter_op_threads ?? 1), 1, 64),
      parallelExecution: Boolean(inf.parallelExecution ?? inf.parallel_execution ?? false),
      enableMemoryPattern: Boolean(
        inf.enableMemoryPattern ?? inf.enable_memory_pattern ?? true,
      ),
      ortProfiling: Boolean(inf.ortProfiling ?? inf.ort_profiling ?? false),
      ortProfilingMaxDecodes: clampInt(
        Number(inf.ortProfilingMaxDecodes ?? inf.ort_profiling_max_decodes ?? 3),
        1,
        50,
      ),
      keepModelLoaded: Boolean(inf.keepModelLoaded ?? inf.keep_model_loaded ?? false),
      cudaFallbackToCpu: Boolean(inf.cudaFallbackToCpu ?? inf.cuda_fallback_to_cpu ?? true),
    },
    realtime: readRealtime(raw.realtime as Record<string, unknown> | undefined),
    vad: readVad(raw.vad as Record<string, unknown> | undefined),
    recognition: readRecognition(raw.recognition as Record<string, unknown> | undefined),
    microphone: {
      deviceId: String(mic.deviceId ?? mic.device_id ?? ""),
      sampleRate: Number(mic.sampleRate ?? mic.sample_rate ?? 16000),
    },
  };
}

function coerceOptionalMs(value: unknown): number | null {
  if (value === "" || value == null) return null;
  const n = Number(value);
  if (!Number.isFinite(n) || n <= 0) return null;
  return n;
}

function clampInt(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(max, Math.max(min, Math.trunc(value)));
}

function serializeConfig(config: LocalAsrConfig): Record<string, unknown> {
  const family = parseModelFamily(config.model?.family ?? "parakeet_tdt");
  return {
    model: {
      family,
      variant: config.model?.variant ?? defaultVariantForFamily(family),
      targetLang: (config.model?.targetLang ?? "auto").trim() || "auto",
      path: config.model?.path ?? "",
      manifestSha256: config.model?.manifestSha256 ?? "",
    },
    inference: {
      executionProvider: config.inference.executionProvider,
      graphOptimizationLevel: clampInt(Number(config.inference.graphOptimizationLevel ?? 1), 0, 3),
      intraOpThreads: clampInt(Number(config.inference.intraOpThreads ?? 4), 1, 64),
      interOpThreads: clampInt(Number(config.inference.interOpThreads ?? 1), 1, 64),
      parallelExecution: Boolean(config.inference.parallelExecution ?? false),
      enableMemoryPattern: Boolean(config.inference.enableMemoryPattern ?? true),
      ortProfiling: Boolean(config.inference.ortProfiling ?? false),
      ortProfilingMaxDecodes: clampInt(Number(config.inference.ortProfilingMaxDecodes ?? 3), 1, 50),
      keepModelLoaded: Boolean(config.inference.keepModelLoaded ?? false),
      cudaFallbackToCpu: Boolean(config.inference.cudaFallbackToCpu ?? true),
    },
    realtime: {
      latencyPreset: config.realtime.latencyPreset,
      streamingDecode: config.realtime.streamingDecode,
      partialEmitMode: config.realtime.partialEmitMode,
      partialMinNewWords: config.realtime.partialMinNewWords,
      partialMinDeltaChars: config.realtime.partialMinDeltaChars,
      partialCoalescingMs: config.realtime.partialCoalescingMs,
      decodeIntervalMs: coerceOptionalMs(config.realtime.decodeIntervalMs),
      windowMs: coerceOptionalMs(config.realtime.windowMs),
      segmentEnqueueDeltaMs: coerceOptionalMs(config.realtime.segmentEnqueueDeltaMs),
      firstPartialMinSpeechMs: coerceOptionalMs(config.realtime.firstPartialMinSpeechMs),
    },
    vad: {
      enabled: config.vad.enabled,
      vadMode: config.vad.vadMode,
      energyGateEnabled: config.vad.energyGateEnabled,
      minRmsForRecognition: config.vad.minRmsForRecognition,
      minVoicedRatio: config.vad.minVoicedRatio,
      speechAttackFrames: config.vad.speechAttackFrames,
      speechPrerollFrames: config.vad.speechPrerollFrames,
      partialEmitIntervalMs: coerceOptionalMs(config.vad.partialEmitIntervalMs),
      speechThreshold: config.vad.speechThreshold,
      minSpeechMs: config.vad.minSpeechMs,
      minSilenceMs: config.vad.minSilenceMs,
      silenceHoldMs: config.vad.silenceHoldMs,
      speechPadMs: config.vad.speechPadMs,
      maxSegmentMs: config.vad.maxSegmentMs,
    },
    recognition: {
      inputGain: config.recognition.inputGain,
      preemphasisEnabled: config.recognition.preemphasisEnabled,
      preemphasisCoeff: config.recognition.preemphasisCoeff,
      noiseGateEnabled: config.recognition.noiseGateEnabled,
      noiseGateThreshold: config.recognition.noiseGateThreshold,
      hallucinationFilterEnabled: config.recognition.hallucinationFilterEnabled,
      hallucinationMinChars: config.recognition.hallucinationMinChars,
      hallucinationCooldownMs: config.recognition.hallucinationCooldownMs,
    },
    microphone: {
      deviceId: config.microphone.deviceId,
      sampleRate: config.microphone.sampleRate ?? 16000,
    },
  };
}

function readInputDevice(raw: Record<string, unknown>): InputDeviceInfo {
  return {
    id: String(raw.id ?? ""),
    label: String(raw.label ?? raw.id ?? ""),
    isDefault: Boolean(raw.isDefault ?? raw.is_default),
  };
}

function readDllGroup(raw: Record<string, unknown> | undefined): DllGroupStatus {
  const g = raw ?? {};
  return {
    ok: Boolean(g.ok),
    missing: Array.isArray(g.missing) ? (g.missing as string[]) : [],
    found: Array.isArray(g.found) ? (g.found as DllGroupStatus["found"]) : [],
    downloadMb: Number(g.downloadMb ?? g.download_mb ?? 0),
  };
}

function readEnv(raw: Record<string, unknown>): LocalAsrEnvCheck {
  return {
    vcruntime: readDllGroup((raw.vcruntime ?? raw.vcRuntime) as Record<string, unknown>),
    ortCpu: readDllGroup((raw.ortCpu ?? raw.ort_cpu) as Record<string, unknown>),
    ortGpu: readDllGroup((raw.ortGpu ?? raw.ort_gpu) as Record<string, unknown>),
    cudaRedist: readDllGroup((raw.cudaRedist ?? raw.cuda_redist) as Record<string, unknown>),
    cudaToolkit: {
      ok: Boolean((raw.cudaToolkit as Record<string, unknown> | undefined)?.ok ?? (raw.cuda_toolkit as Record<string, unknown> | undefined)?.ok),
      version: ((raw.cudaToolkit ?? raw.cuda_toolkit) as Record<string, unknown> | undefined)?.version as string | undefined,
      message: ((raw.cudaToolkit ?? raw.cuda_toolkit) as Record<string, unknown> | undefined)?.message as string | undefined,
    },
    cpuDepsReady: Boolean(raw.cpuDepsReady ?? raw.cpu_deps_ready),
    cudaDepsReady: Boolean(raw.cudaDepsReady ?? raw.cuda_deps_ready),
  };
}

const MODEL_FAMILIES: ModelFamily[] = ["parakeet_tdt"];

export { MODEL_FAMILIES };

export function parseModelFamily(_raw: string): ModelFamily {
  return "parakeet_tdt";
}

export function defaultVariantForFamily(_family: ModelFamily = "parakeet_tdt"): ModelVariant {
  return "int8";
}

function readModelCatalog(raw: Record<string, unknown>): ModelCatalogEntry[] {
  const list = Array.isArray(raw.models) ? raw.models : [];
  return list
    .map((entry) => {
      const item = (entry ?? {}) as Record<string, unknown>;
      const variant = String(item.variant ?? "").trim();
      if (!variant) return null;
      return {
        family: parseModelFamily(String(item.family ?? "")),
        variant,
        installed: Boolean(item.installed),
        sizeMb: Number(item.sizeMb ?? item.size_mb ?? 0),
        active: Boolean(item.active),
        sourceAuthor: String(item.sourceAuthor ?? item.source_author ?? ""),
      } satisfies ModelCatalogEntry;
    })
    .filter((entry): entry is ModelCatalogEntry => entry !== null);
}

export function defaultModelCatalog(_family: ModelFamily = "parakeet_tdt"): ModelCatalogEntry[] {
  return [
    {
      family: "parakeet_tdt",
      variant: "int8",
      installed: false,
      sizeMb: 670,
      active: true,
      sourceAuthor: "istupakov",
    },
    {
      family: "parakeet_tdt",
      variant: "fp32",
      installed: false,
      sizeMb: 2500,
      active: false,
      sourceAuthor: "istupakov",
    },
    {
      family: "parakeet_tdt",
      variant: "int8_smoothquant",
      installed: false,
      sizeMb: 900,
      active: false,
      sourceAuthor: "Olicorne",
    },
  ];
}

function readSetupChecklist(raw: Record<string, unknown> | undefined): LocalAsrSetupChecklist {
  const setup = raw ?? {};
  return {
    depsReady: Boolean(setup.depsReady ?? setup.deps_ready),
    modelInstalled: Boolean(setup.modelInstalled ?? setup.model_installed),
    micTestPassed: Boolean(setup.micTestPassed ?? setup.mic_test_passed),
    parakeetFinalReceived: Boolean(setup.parakeetFinalReceived ?? setup.parakeet_final_received),
    setupComplete: Boolean(setup.setupComplete ?? setup.setup_complete),
  };
}

function normalizeStatus(raw: Record<string, unknown>): LocalAsrModuleStatus {
  const envRaw = (raw.env ?? {}) as Record<string, unknown>;
  const activeFamily = parseModelFamily(
    String(raw.activeModelFamily ?? raw.active_model_family ?? "parakeet_tdt"),
  );
  const models = readModelCatalog(raw);
  const activeRaw = String(raw.activeModelVariant ?? raw.active_model_variant ?? defaultVariantForFamily(activeFamily));
  return {
    phase: String(raw.phase ?? "unconfigured"),
    ready: Boolean(raw.ready),
    cudaReady: Boolean(raw.cudaReady ?? raw.cuda_ready),
    depsReady: Boolean(raw.depsReady ?? raw.deps_ready),
    executionProvider: String(raw.executionProvider ?? raw.execution_provider ?? "cpu"),
    activeExecutionProvider: String(
      raw.activeExecutionProvider ?? raw.active_execution_provider ?? "cpu",
    ),
    message: String(raw.message ?? ""),
    lastError: (raw.lastError ?? raw.last_error ?? null) as string | null,
    env: readEnv(envRaw),
    modelInstalled: Boolean(raw.modelInstalled ?? raw.model_installed),
    modelLoaded: Boolean(raw.modelLoaded ?? raw.model_loaded),
    modelLoadMs:
      raw.modelLoadMs != null || raw.model_load_ms != null
        ? Number(raw.modelLoadMs ?? raw.model_load_ms)
        : null,
    probeCpuOk: readOptionalBool(raw.probeCpuOk ?? raw.probe_cpu_ok),
    probeCudaOk: readOptionalBool(raw.probeCudaOk ?? raw.probe_cuda_ok),
    ortProfilingActive: Boolean(raw.ortProfilingActive ?? raw.ort_profiling_active),
    ortProfilingDecodeCount: Number(
      raw.ortProfilingDecodeCount ?? raw.ort_profiling_decode_count ?? 0,
    ),
    ortProfilingMaxDecodes: Number(
      raw.ortProfilingMaxDecodes ?? raw.ort_profiling_max_decodes ?? 0,
    ),
    ortProfilingStoppedBudget: Boolean(
      raw.ortProfilingStoppedBudget ?? raw.ort_profiling_stopped_budget,
    ),
    lastOrtProfilePath: (raw.lastOrtProfilePath ?? raw.last_ort_profile_path ?? null) as
      | string
      | null,
    lastDecodeTiming: readDecodeTiming(raw.lastDecodeTiming ?? raw.last_decode_timing),
    activeModelFamily: activeFamily,
    activeModelVariant: activeRaw,
    models: models.length > 0 ? models : defaultModelCatalog(activeFamily),
    setup: readSetupChecklist((raw.setup ?? {}) as Record<string, unknown>),
  };
}

export async function fetchLocalAsrStatus(): Promise<LocalAsrModuleStatus> {
  const res = await apiFetch("/api/asr/local/status");
  const body = (await res.json()) as { status: Record<string, unknown> };
  return normalizeStatus(body.status);
}

export async function refreshLocalAsrDeps(): Promise<LocalAsrModuleStatus> {
  const res = await apiFetch("/api/asr/local/deps/check", { method: "POST" });
  const body = (await res.json()) as { status: Record<string, unknown> };
  return normalizeStatus(body.status);
}

export async function downloadLocalAsrDep(kind: DepDownloadKind): Promise<LocalAsrModuleStatus> {
  const res = await apiFetch("/api/asr/local/deps/download", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ kind }),
  });
  const body = (await res.json()) as { ok?: boolean; message?: string; status?: Record<string, unknown> };
  if (!res.ok || body.ok === false) {
    throw new Error(body.message || `download failed (${res.status})`);
  }
  return normalizeStatus(body.status ?? {});
}

export async function downloadLocalAsrModel(
  variant: ModelVariant,
  family: ModelFamily = "parakeet_tdt",
): Promise<LocalAsrModuleStatus> {
  const res = await apiFetch("/api/asr/local/model/download", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ variant, family }),
  });
  const body = (await res.json()) as { ok?: boolean; message?: string; status?: Record<string, unknown> };
  if (!res.ok || body.ok === false) {
    throw new Error(body.message || `model download failed (${res.status})`);
  }
  return normalizeStatus(body.status ?? {});
}

export async function selectLocalAsrModel(
  variant: ModelVariant,
  family: ModelFamily = "parakeet_tdt",
): Promise<LocalAsrModuleStatus> {
  const res = await apiFetch("/api/asr/local/model/select", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ variant, family }),
  });
  const body = (await res.json()) as { ok?: boolean; message?: string; status?: Record<string, unknown> };
  if (!res.ok || body.ok === false) {
    throw new Error(body.message || `model select failed (${res.status})`);
  }
  return normalizeStatus(body.status ?? {});
}

export async function loadLocalAsrConfig(): Promise<LocalAsrConfig> {
  const res = await apiFetch("/api/asr/local/config");
  const body = (await res.json()) as { config: Record<string, unknown> };
  return normalizeConfig(body.config ?? {});
}

export async function saveLocalAsrConfig(config: LocalAsrConfig): Promise<LocalAsrConfig> {
  const res = await apiFetch("/api/asr/local/config/save", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ config: serializeConfig(config) }),
  });
  if (!res.ok) {
    const body = (await res.json()) as { message?: string };
    throw new Error(body.message || `save failed (${res.status})`);
  }
  const body = (await res.json()) as { config?: Record<string, unknown> };
  return normalizeConfig(body.config ?? {});
}

export async function fetchLocalAsrTransfer(): Promise<TransferProgress> {
  const res = await apiFetch("/api/asr/local/transfer");
  const body = (await res.json()) as { transfer?: Record<string, unknown> };
  return readTransfer(body.transfer);
}

export async function cancelLocalAsrTransfer(): Promise<TransferProgress> {
  const res = await apiFetch("/api/asr/local/transfer/cancel", { method: "POST" });
  const body = (await res.json()) as { ok?: boolean; message?: string; transfer?: Record<string, unknown> };
  if (!res.ok || body.ok === false) {
    throw new Error(body.message || `cancel failed (${res.status})`);
  }
  return readTransfer(body.transfer);
}

export async function deleteLocalAsrDep(kind: DepDownloadKind): Promise<LocalAsrModuleStatus> {
  const res = await apiFetch("/api/asr/local/deps/delete", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ kind }),
  });
  const body = (await res.json()) as { ok?: boolean; message?: string; status?: Record<string, unknown> };
  if (!res.ok || body.ok === false) {
    throw new Error(body.message || `delete failed (${res.status})`);
  }
  return normalizeStatus(body.status ?? {});
}

export async function deleteLocalAsrModel(
  variant: ModelVariant,
  family: ModelFamily = "parakeet_tdt",
): Promise<LocalAsrModuleStatus> {
  const res = await apiFetch("/api/asr/local/model/delete", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ variant, family }),
  });
  const body = (await res.json()) as { ok?: boolean; message?: string; status?: Record<string, unknown> };
  if (!res.ok || body.ok === false) {
    throw new Error(body.message || `model delete failed (${res.status})`);
  }
  return normalizeStatus(body.status ?? {});
}

export async function fetchCudaToolkitUrl(): Promise<string> {
  const res = await apiFetch("/api/asr/local/driver-url");
  const body = (await res.json()) as { url: string };
  return body.url;
}

export async function openExternalHttpsUrl(url: string): Promise<void> {
  const trimmed = url.trim();
  if (!trimmed) {
    return;
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("open_external_https_url", { url: trimmed });
  } catch {
    window.open(trimmed, "_blank", "noopener,noreferrer");
  }
}

export async function probeLocalAsrProvider(
  provider: "cpu" | "cuda",
): Promise<{ probe: ProbeResult; status: LocalAsrModuleStatus }> {
  const res = await apiFetch("/api/asr/local/deps/probe", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ provider }),
  });
  const body = (await res.json()) as {
    ok?: boolean;
    message?: string;
    probe?: Record<string, unknown>;
    status?: Record<string, unknown>;
  };
  if (!res.ok || body.ok === false) {
    throw new Error(body.message || `probe failed (${res.status})`);
  }
  const probeRaw = (body.probe ?? {}) as Record<string, unknown>;
  return {
    probe: {
      provider: String(probeRaw.provider ?? provider),
      ok: Boolean(probeRaw.ok),
      loadMs: Number(probeRaw.loadMs ?? probeRaw.load_ms ?? 0),
      message: String(probeRaw.message ?? ""),
      fallbackProvider: (probeRaw.fallbackProvider ?? probeRaw.fallback_provider ?? null) as string | null,
    },
    status: normalizeStatus(body.status ?? {}),
  };
}

function networkErrorMessage(err: unknown): string {
  if (err instanceof TypeError) {
    return "network";
  }
  return err instanceof Error ? err.message : String(err);
}

export async function loadLocalAsrModel(): Promise<{ load: LoadResult; status: LocalAsrModuleStatus }> {
  let res: Response;
  try {
    res = await apiFetch("/api/asr/local/model/load", { method: "POST" });
  } catch (err) {
    const kind = networkErrorMessage(err);
    throw new Error(kind === "network" ? "network" : kind);
  }
  const body = (await res.json()) as {
    ok?: boolean;
    message?: string;
    load?: Record<string, unknown>;
    status?: Record<string, unknown>;
  };
  if (!res.ok || body.ok === false) {
    throw new Error(body.message || `load failed (${res.status})`);
  }
  const loadRaw = (body.load ?? {}) as Record<string, unknown>;
  return {
    load: {
      loaded: Boolean(loadRaw.loaded),
      loadMs: Number(loadRaw.loadMs ?? loadRaw.load_ms ?? 0),
      activeExecutionProvider: String(
        loadRaw.activeExecutionProvider ?? loadRaw.active_execution_provider ?? "cpu",
      ),
      message: String(loadRaw.message ?? ""),
    },
    status: normalizeStatus(body.status ?? {}),
  };
}

export async function unloadLocalAsrModel(): Promise<LocalAsrModuleStatus> {
  const res = await apiFetch("/api/asr/local/model/unload", { method: "POST" });
  const body = (await res.json()) as { ok?: boolean; message?: string; status?: Record<string, unknown> };
  if (!res.ok || body.ok === false) {
    throw new Error(body.message || `unload failed (${res.status})`);
  }
  return normalizeStatus(body.status ?? {});
}

export async function listLocalAsrMicrophones(): Promise<InputDeviceInfo[]> {
  const res = await apiFetch("/api/asr/local/mics/list");
  const body = (await res.json()) as { ok?: boolean; message?: string; devices?: Record<string, unknown>[] };
  if (!res.ok || body.ok === false) {
    throw new Error(body.message || `mic list failed (${res.status})`);
  }
  return (body.devices ?? []).map((entry) => readInputDevice(entry));
}

export async function startLocalAsrTest(deviceId?: string): Promise<TestBenchSnapshot> {
  const res = await apiFetch("/api/asr/local/test/start", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      duration_ms: 0,
      device_id: deviceId ?? "",
    }),
  });
  const body = (await res.json()) as { ok?: boolean; message?: string; test?: Record<string, unknown> };
  if (!res.ok || body.ok === false) {
    throw new Error(body.message || `test start failed (${res.status})`);
  }
  return readTestBench(body.test);
}

export async function stopLocalAsrTest(): Promise<TestBenchSnapshot> {
  const res = await apiFetch("/api/asr/local/test/stop", { method: "POST" });
  const body = (await res.json()) as { ok?: boolean; message?: string; test?: Record<string, unknown> };
  if (!res.ok || body.ok === false) {
    throw new Error(body.message || `test stop failed (${res.status})`);
  }
  return readTestBench(body.test);
}

export async function fetchLocalAsrTestStatus(): Promise<TestBenchSnapshot> {
  const res = await apiFetch("/api/asr/local/test/status");
  const body = (await res.json()) as { test?: Record<string, unknown> };
  return readTestBench(body.test);
}
