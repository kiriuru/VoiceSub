/** Shared browser ASR worker state (page glue + session manager). */

import type { SpeechRecognitionConstructor, WorkerSpeechRecognition } from "./speech-types";

export type { SpeechRecognitionConstructor, WorkerSpeechRecognition };

export interface BrowserLifecycleConfig {
  minimumReconnectIntervalMs: number;
  normalRestartDelayMs: number;
  noSpeechRestartDelayMs: number;
  networkReconnectInitialMs: number;
  networkReconnectMaxMs: number;
  stuckStoppingTimeoutMs: number;
  maxBrowserSessionAgeMs: number;
  prepareCycleBeforeMs: number;
  forceFinalOnInterruption: boolean;
  forceFinalMinChars: number;
  forceFinalMinStableMs: number;
  overlapPrestartAfterMs: number;
  overlapBuddyGhostTimeoutMs: number;
  overlapBuddyGhostActiveMicMs: number;
}

export interface MicrophoneMonitor {
  stream: MediaStream;
  track: MediaStreamTrack;
  audioContext: AudioContext | null;
  analyser: AnalyserNode | null;
  sourceNode: MediaStreamAudioSourceNode | null;
  rmsBuffer: Uint8Array | null;
  intervalId: ReturnType<typeof setInterval> | null;
}

export interface LastForcedFinal {
  generation_id: number;
  client_segment_id: string | null;
  text: string;
  reason?: string;
  at_ms: number;
}

export interface BrowserAsrState {
  desiredRunning: boolean;
  pendingStart: boolean;
  generationId: number;
  sessionId: string;
  providerName: string;
  browserMode: string;
  browserSupervisorState: string;
  recognitionState: string;
  restartTimer: ReturnType<typeof setTimeout> | null;
  reconnectTimer: ReturnType<typeof setTimeout> | null;
  watchdogTimerId: ReturnType<typeof setInterval> | null;
  restartCount: number;
  noSpeechCount: number;
  networkErrorCount: number;
  websocketReady: boolean;
  stoppingSinceMs: number | null;
  lastStartAtMs: number;
  lastEndAtMs: number;
  lastSessionStartedAtMs: number;
  lastSessionEndedAtMs: number;
  lastEventAtMs: number;
  lastResultAtMs: number;
  lastResultIndex: number | null;
  browserCyclePending: boolean;
  browserCycleCount: number;
  browserMinimumReconnectSuppressedCount: number;
  browserForcedFinalOnInterruptionCount: number;
  lastErrorKind: string | null;
  lastError: string | null;
  degradedReason: string | null;
  terminalDegradedReason: string | null;
  healthDegradedReason: string | null;
  socketDegraded: boolean;
  visibilityDegraded: boolean;
  restartBackoffMs: number;
  noSpeechBackoffMs: number;
  pendingRestartReason: string | null;
  lastRestartReason: string | null;
  recognition: WorkerSpeechRecognition | null;
  recognitionOverlapSlots: WorkerSpeechRecognition[] | null;
  recognitionOverlapActiveSlot: number | null;
  recognitionOverlapPrestarted: boolean;
  recognitionOverlapSlotListening: boolean[] | null;
  recognitionOverlapPrestartTimer: ReturnType<typeof setTimeout> | null;
  recognitionOverlapActiveListenSinceMs: number | null;
  recognitionOverlapSlotListenSinceMs: [number | null, number | null] | null;
  recognitionOverlapSlotActivityAtMs: [number | null, number | null] | null;
  recognitionOverlapActiveSpeechPrestartDone: boolean;
  webSpeechPhraseHintsSuppressed: boolean;
  webSpeechLanguageSoftFallbackUsed: boolean;
  recognitionGenerationId: number;
  effectiveContinuousMode: string;
  currentClientSegmentId: string | null;
  nextClientSegmentOrdinal: number;
  currentSegmentLastPartialText: string;
  currentSegmentLastFinalText: string;
  currentPartialStableSinceMs: number;
  currentSegmentForcedFinalized: boolean;
  lastForcedFinal: LastForcedFinal | null;
  duplicatePartialSuppressed: number;
  duplicateFinalSuppressed: number;
  lateForcedFinalSuppressed: number;
  minimumReconnectIntervalMs: number;
  normalRestartDelayMs: number;
  noSpeechRestartDelayMs: number;
  networkReconnectInitialMs: number;
  networkReconnectMaxMs: number;
  maxBrowserSessionAgeMs: number;
  networkErrorBurstCount: number;
  networkErrorBurstStartedAtMs: number;
  lastNetworkPreflightAtMs: number;
  lastNetworkPreflightOk: boolean | null;
  networkPreflightInFlight: boolean;
  wakeLockActive: boolean;
  wakeLockSupported: boolean;
  prepareCycleBeforeMs: number;
  forceFinalOnInterruption: boolean;
  forceFinalMinChars: number;
  forceFinalMinStableMs: number;
  micTrackReadyState: string | null;
  micTrackMuted: boolean;
  micRms: number;
  micActiveRecentMs: number | null;
  lastMicActivityAt: number;
  getUserMediaCount: number;
  getUserMediaLastError: string | null;
  micStreamActive: boolean;
  mediaTracksStoppedCount: number;
  mediaTrackLeakGuardCount: number;
  workerTranscriptMessageSequence: number;
  configuredLanguage: string;
  sourceLang: string;
  socket: WebSocket | null;
  forceFinalizeTimer: ReturnType<typeof setTimeout> | null;
  currentPartial: string;
  approxCount: number;
  finalCount: number;
  missingFinalCount: number;
  forcedCount: number;
  appSendCount: number;
  onSound: boolean;
  hasOpenSentence: boolean;
  forceFinalizationTimeoutMs: number;
  actualContinuous: boolean;
  mediaStream: MediaStream | null;
  stuckStoppingTimeoutMs: number;
  browserLifecycleConfig: BrowserLifecycleConfig;
  currentConfigPayload: Record<string, unknown> | null;
  microphoneMonitor: MicrophoneMonitor | null;
  micHealthUpdatedAt: number;
  settingsSavePromise: Promise<void>;
}

export interface RecognitionSettings {
  language?: string;
  interimResults?: boolean;
  continuous?: boolean;
  providerName?: string;
  overlap_recognition_sessions?: boolean;
  minimumReconnectIntervalMs?: number;
  normalRestartDelayMs?: number;
  noSpeechRestartDelayMs?: number;
  networkReconnectInitialMs?: number;
  networkReconnectMaxMs?: number;
  stuckStoppingTimeoutMs?: number;
  maxBrowserSessionAgeMs?: number;
  prepareCycleBeforeMs?: number;
  forceFinalOnInterruption?: boolean;
  forceFinalMinChars?: number;
  forceFinalMinStableMs?: number;
}

export interface TimingLimits {
  restartDelayByReasonMs: Record<string, number>;
  initialNoSpeechDelayMs: number;
  maxNoSpeechDelayMs: number;
  initialNetworkBackoffMs: number;
  maxNetworkBackoffMs: number;
  networkPreflightBurstThreshold: number;
  networkPreflightBurstWindowMs: number;
  networkPreflightCooldownMs: number;
  micSilentDegradedAfterMs: number;
  voiceBelowRecognitionRmsThreshold: number;
  voiceBelowRecognitionGraceMs: number;
  voiceBelowRecognitionMicWindowMs: number;
  voiceBelowRecognitionMinNoSpeech: number;
  stallDegradedAfterMs: number;
  recentMicActivityWindowMs: number;
}

export interface SessionManagerOptions {
  state: BrowserAsrState;
  SpeechRecognitionCtor: SpeechRecognitionConstructor | null;
  locale?: () => string;
  translate?: (key: string, vars?: Record<string, string | number>) => string;
  appendLog?: (message: string) => void;
  setStatus?: (status: string) => void;
  updateCounters?: () => void;
  ensureMicrophonePermission?: () => Promise<MediaStream>;
  getRecognitionSettings?: () => RecognitionSettings;
  isForceFinalizationEnabled?: () => boolean;
  setPartialText?: (value: string) => void;
  setFinalText?: (value: string) => void;
  loadBackendSettings?: () => Promise<void>;
}

export interface ClassifiedRecognitionError {
  kind: string;
  errorKind: string;
  errorMessage: string;
  logKey?: string;
  terminalReason?: string;
}

/** Host surface used by ASR lifecycle modules (avoids circular imports). */
export interface AsrManagerHost {
  state: BrowserAsrState;
  SpeechRecognitionCtor: SpeechRecognitionConstructor | null;
  restartDelayByReasonMs: Record<string, number>;
  initialNoSpeechDelayMs: number;
  maxNoSpeechDelayMs: number;
  initialNetworkBackoffMs: number;
  maxNetworkBackoffMs: number;
  networkPreflightBurstThreshold: number;
  networkPreflightBurstWindowMs: number;
  networkPreflightCooldownMs: number;
  networkPreflightTimeoutMs: number;
  micSilentDegradedAfterMs: number;
  voiceBelowRecognitionRmsThreshold: number;
  voiceBelowRecognitionGraceMs: number;
  voiceBelowRecognitionMicWindowMs: number;
  voiceBelowRecognitionMinNoSpeech: number;
  stallDegradedAfterMs: number;
  recentMicActivityWindowMs: number;
  watchdogIntervalMs: number;
  maxStoppingMs: number;
  visibleIdleRestartMs: number;
  hiddenIdleRestartMs: number;
  maxBrowserSessionAgeMs: number;
  prepareCycleBeforeMs: number;
  minimumReconnectIntervalMs: number;
  recognitionStartLogMinGapMs: number;
  _lastWebSpeechNetworkHintAtMs?: number;
  _permissionPromise: Promise<unknown> | null;
  _appendLogThrottleState: Map<string, number> | null;
  _wakeLockSentinel: WakeLockSentinel | null;
  _wakeLockBound: boolean;
  _wakeLockRetryTimer: ReturnType<typeof setTimeout> | null;
  options: SessionManagerOptions;
  appendLogInternal(message: string): void;
  appendLogThrottledInternal(message: string, throttleKey: string | null, minGapMs: number): void;
  now(): number;
  translate(key: string, vars?: Record<string, string | number>): string;
  locale(): string;
  getRecognitionSettings(): RecognitionSettings;
  webSpeechPolicy(): typeof import("./web-speech-policy").webSpeechRecognitionPolicy;
  isForceFinalizationEnabled(): boolean;
  timingLimits(): TimingLimits;
  setStatusInternal(status: string): void;
  updateCountersInternal(): void;
  setSupervisorStateInternal(nextState: string): void;
  setRecognitionStateInternal(nextState: string): void;
  setDegradedReasonInternal(reason: string | null): void;
  setTerminalDegradedReasonInternal(reason: string | null): void;
  setHealthDegradedReasonInternal(reason: string | null): void;
  refreshDegradedReasonInternal(): void;
  setLastErrorInternal(kind: string | null, message: string | null): void;
  markActivityInternal(label: string): void;
  resetSegmentTrackingInternal(): void;
  currentGenerationIdInternal(): number;
  ensureClientSegmentIdInternal(): string | null;
  consumeCompletedSegmentInternal(): void;
  normalizeTranscriptTextInternal(value: string): string;
  restartDelayForReasonInternal(reason: string): number;
  shouldSuppressDuplicatePartialInternal(text: string): boolean;
  shouldSuppressFinalInternal(text: string, options?: { forcedFinal?: boolean }): boolean;
  buildUpdatePayloadInternal(payload: Record<string, unknown>): Record<string, unknown>;
  buildWorkerPayloadInternal(type: string, extra?: Record<string, unknown>): Record<string, unknown>;
  emitWorkerStatus(reason: string): boolean;
  emitHeartbeat(reason: string): void;
  sendUpdateInternal(payload: Record<string, unknown>): boolean;
  scheduleForceFinalizeInternal(): void;
  clearForceFinalizeTimerInternal(): void;
  clearRestartTimerInternal(): void;
  clearReconnectTimerInternal(): void;
  clearAllTimersInternal(): void;
  recognitionStartBurstThrottleInternal(reason: string): { gapMs: number; key: string | null };
  minimumReconnectGuardDelayMsInternal(delayMs: number): number;
  canForceFinalizeOnInterruptionInternal(): boolean;
  forceFinalizeOnInterruptionInternal(reason: string): boolean;
  refreshHealthSignalsInternal(): void;
  stripWebSpeechExperimentalHints(recognition: WorkerSpeechRecognition): void;
  applyChromeCompatHintsToRecognition(recognition: WorkerSpeechRecognition): void;
  wireRecognitionHandlers(
    recognition: WorkerSpeechRecognition,
    generationId: number,
    overlapSlotIndex: number | null
  ): void;
  isActiveGeneration(generationId: number): boolean;
  cleanupRecognitionInstance(generationId: number): void;
  recognitionOverlapActiveInternal(): boolean;
  applyRecognitionSettings(): void;
  stopInternal(): void;
  scheduleRestartInternal(reason: string, options?: { backoffMs?: number }): void;
  performControlledStartInternal(reason: string): void;
  transitionToStoppingInternal(reason: string): void;
  ensureSocketConnectedInternal(): void;
  handleSocketMessageInternal(raw: string): void;
}
