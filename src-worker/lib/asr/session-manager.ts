import type {
  AsrManagerHost,
  BrowserAsrState,
  RecognitionSettings,
  SessionManagerOptions,
  SpeechRecognitionConstructor,
  WorkerSpeechRecognition,
} from "./types";
import { applyInstanceDefaults, initializeBrowserAsrState } from "./session-state";
import {
  recordThrottledAppendLog,
  recognitionStartBurstThrottle,
  shouldThrottleAppendLog,
} from "./log-throttle-logic";
import { resolveDegradedReason } from "./degraded-reason-logic";
import {
  buildTranscriptUpdatePayload,
  canForceFinalizeOnInterruption,
  consumeCompletedSegment,
  currentGenerationId,
  ensureClientSegmentId,
  markResultActivity,
  normalizeTranscriptText,
  resetSegmentTrackingFields,
  shouldSuppressDuplicatePartial,
  shouldSuppressFinal,
} from "./transcript-logic";
import {
  currentSessionAgeMs,
  minimumReconnectGuardDelayMs,
  resetNetworkErrorBurst,
  restartDelayForReason,
} from "./restart-timing-logic";
import { computeHealthDegradedReason } from "./health-signals-logic";
import { evaluateWatchdogTick } from "./watchdog-logic";
import { buildWorkerPayload } from "./worker-payload-logic";
import { ensureSocketConnected, parseBrowserAsrControlMessage } from "./socket-bridge";
import { waitUntilDocumentVisibleForRecognition } from "./visibility-wait-logic";
import { acquireWakeLock, hasWakeLockSupport, releaseWakeLock } from "./wake-lock-bridge";
import { ensureMicrophonePermission } from "./mic-permission-bridge";
import {
  cleanupRecognitionInstance,
  performControlledStart,
  scheduleRestart,
  transitionToStopping,
} from "./recognition-lifecycle";
import { wireRecognitionHandlers } from "./recognition-handlers";
import {
  clearOverlapTimeBasedPrestart,
  overlapActiveSlotIndex,
  prestartOverlapBuddyIfNeeded,
  recognitionOverlapActive,
  recoverGhostOverlapBuddy,
} from "./overlap-logic";
import { webSpeechRecognitionPolicy } from "./web-speech-policy";
import { INSTANCE_DEFAULTS } from "./session-defaults";

export class BrowserAsrSessionManager implements AsrManagerHost {
  options: SessionManagerOptions;
  state: BrowserAsrState;
  SpeechRecognitionCtor: SpeechRecognitionConstructor | null;
  restartDelayByReasonMs: Record<string, number> = { ...INSTANCE_DEFAULTS.restartDelayByReasonMs };
  initialNoSpeechDelayMs: number = INSTANCE_DEFAULTS.initialNoSpeechDelayMs;
  maxNoSpeechDelayMs: number = INSTANCE_DEFAULTS.maxNoSpeechDelayMs;
  initialNetworkBackoffMs: number = INSTANCE_DEFAULTS.initialNetworkBackoffMs;
  maxNetworkBackoffMs: number = INSTANCE_DEFAULTS.maxNetworkBackoffMs;
  watchdogIntervalMs: number = INSTANCE_DEFAULTS.watchdogIntervalMs;
  maxStoppingMs: number = INSTANCE_DEFAULTS.maxStoppingMs;
  visibleIdleRestartMs: number = INSTANCE_DEFAULTS.visibleIdleRestartMs;
  hiddenIdleRestartMs: number = INSTANCE_DEFAULTS.hiddenIdleRestartMs;
  stallDegradedAfterMs: number = INSTANCE_DEFAULTS.stallDegradedAfterMs;
  micSilentDegradedAfterMs: number = INSTANCE_DEFAULTS.micSilentDegradedAfterMs;
  recentMicActivityWindowMs: number = INSTANCE_DEFAULTS.recentMicActivityWindowMs;
  minimumReconnectIntervalMs: number = INSTANCE_DEFAULTS.minimumReconnectIntervalMs;
  maxBrowserSessionAgeMs: number = INSTANCE_DEFAULTS.maxBrowserSessionAgeMs;
  prepareCycleBeforeMs: number = INSTANCE_DEFAULTS.prepareCycleBeforeMs;
  networkPreflightBurstThreshold: number = INSTANCE_DEFAULTS.networkPreflightBurstThreshold;
  networkPreflightBurstWindowMs: number = INSTANCE_DEFAULTS.networkPreflightBurstWindowMs;
  networkPreflightTimeoutMs: number = INSTANCE_DEFAULTS.networkPreflightTimeoutMs;
  networkPreflightCooldownMs: number = INSTANCE_DEFAULTS.networkPreflightCooldownMs;
  voiceBelowRecognitionRmsThreshold: number = INSTANCE_DEFAULTS.voiceBelowRecognitionRmsThreshold;
  voiceBelowRecognitionGraceMs: number = INSTANCE_DEFAULTS.voiceBelowRecognitionGraceMs;
  voiceBelowRecognitionMicWindowMs: number = INSTANCE_DEFAULTS.voiceBelowRecognitionMicWindowMs;
  voiceBelowRecognitionMinNoSpeech: number = INSTANCE_DEFAULTS.voiceBelowRecognitionMinNoSpeech;
  recognitionStartLogMinGapMs: number = INSTANCE_DEFAULTS.recognitionStartLogMinGapMs;
  _lastWebSpeechNetworkHintAtMs?: number;
  _permissionPromise: Promise<unknown> | null = null;
  _appendLogThrottleState: Map<string, number> | null = null;
  _wakeLockSentinel: WakeLockSentinel | null = null;
  _wakeLockBound = false;
  _wakeLockRetryTimer: ReturnType<typeof setTimeout> | null = null;
  private _watchdogTimer: ReturnType<typeof setInterval> | null = null;

  constructor(options: SessionManagerOptions) {
    this.options = options;
    this.state = options.state;
    this.SpeechRecognitionCtor = options.SpeechRecognitionCtor || null;
    applyInstanceDefaults(this as unknown as Record<string, unknown> & { restartDelayByReasonMs: Record<string, number> });
    initializeBrowserAsrState(this.state, this.state);
  }

  appendLogInternal(message: string): void {
    this.options.appendLog?.(message);
  }

  appendLogThrottledInternal(message: string, throttleKey: string | null, minGapMs: number): void {
    if (!throttleKey || !minGapMs) {
      this.appendLogInternal(message);
      return;
    }
    const now = this.now();
    if (!this._appendLogThrottleState) {
      this._appendLogThrottleState = new Map();
    }
    if (shouldThrottleAppendLog(this._appendLogThrottleState, throttleKey, minGapMs, now)) {
      return;
    }
    this.appendLogInternal(message);
    recordThrottledAppendLog(this._appendLogThrottleState, throttleKey, now);
  }

  now(): number {
    return Date.now();
  }

  locale(): string {
    return this.options.locale?.() || "en";
  }

  translate(key: string, vars?: Record<string, string | number>): string {
    if (this.options.translate) {
      return this.options.translate(key, vars);
    }
    return key;
  }

  getRecognitionSettings(): RecognitionSettings {
    return this.options.getRecognitionSettings?.() || {};
  }

  webSpeechPolicy() {
    return webSpeechRecognitionPolicy;
  }

  isForceFinalizationEnabled(): boolean {
    return this.options.isForceFinalizationEnabled?.() !== false;
  }

  timingLimits() {
    return {
      restartDelayByReasonMs: this.restartDelayByReasonMs,
      initialNoSpeechDelayMs: this.initialNoSpeechDelayMs,
      maxNoSpeechDelayMs: this.maxNoSpeechDelayMs,
      initialNetworkBackoffMs: this.initialNetworkBackoffMs,
      maxNetworkBackoffMs: this.maxNetworkBackoffMs,
      networkPreflightBurstThreshold: this.networkPreflightBurstThreshold,
      networkPreflightBurstWindowMs: this.networkPreflightBurstWindowMs,
      networkPreflightCooldownMs: this.networkPreflightCooldownMs,
      micSilentDegradedAfterMs: this.micSilentDegradedAfterMs,
      voiceBelowRecognitionRmsThreshold: this.voiceBelowRecognitionRmsThreshold,
      voiceBelowRecognitionGraceMs: this.voiceBelowRecognitionGraceMs,
      voiceBelowRecognitionMicWindowMs: this.voiceBelowRecognitionMicWindowMs,
      voiceBelowRecognitionMinNoSpeech: this.voiceBelowRecognitionMinNoSpeech,
      stallDegradedAfterMs: this.stallDegradedAfterMs,
      recentMicActivityWindowMs: this.recentMicActivityWindowMs,
    };
  }

  setStatusInternal(status: string): void {
    this.options.setStatus?.(status);
  }

  updateCountersInternal(): void {
    this.options.updateCounters?.();
  }

  setSupervisorStateInternal(nextState: string): void {
    if (this.state.browserSupervisorState === nextState) {
      return;
    }
    this.state.browserSupervisorState = nextState;
    this.emitWorkerStatus("supervisor-state");
    this.updateCountersInternal();
  }

  setRecognitionStateInternal(nextState: string): void {
    this.state.recognitionState = nextState;
    this.updateCountersInternal();
  }

  setDegradedReasonInternal(reason: string | null): void {
    const normalized = String(reason || "").trim() || null;
    if (this.state.degradedReason === normalized) {
      return;
    }
    this.state.degradedReason = normalized;
    this.emitWorkerStatus("degraded");
  }

  setTerminalDegradedReasonInternal(reason: string | null): void {
    this.state.terminalDegradedReason = String(reason || "").trim() || null;
    this.refreshDegradedReasonInternal();
  }

  setHealthDegradedReasonInternal(reason: string | null): void {
    this.state.healthDegradedReason = String(reason || "").trim() || null;
    this.refreshDegradedReasonInternal();
  }

  refreshDegradedReasonInternal(): void {
    this.setDegradedReasonInternal(resolveDegradedReason(this.state));
  }

  setLastErrorInternal(kind: string | null, message: string | null): void {
    this.state.lastErrorKind = String(kind || "")
      .trim()
      .toLowerCase() || null;
    this.state.lastError = String(message || "").trim() || null;
  }

  markActivityInternal(label: string): void {
    const nowMs = this.now();
    if (label === "result") {
      markResultActivity(this.state, nowMs);
      resetNetworkErrorBurst(this.state);
      return;
    }
    this.state.lastEventAtMs = nowMs;
  }

  resetSegmentTrackingInternal(): void {
    resetSegmentTrackingFields(this.state);
    this.clearForceFinalizeTimerInternal();
  }

  currentGenerationIdInternal(): number {
    return currentGenerationId(this.state);
  }

  ensureClientSegmentIdInternal(): string | null {
    return ensureClientSegmentId(this.state);
  }

  consumeCompletedSegmentInternal(): void {
    consumeCompletedSegment(this.state);
  }

  normalizeTranscriptTextInternal(value: string): string {
    return normalizeTranscriptText(value);
  }

  restartDelayForReasonInternal(reason: string): number {
    return restartDelayForReason(this.state, reason, this.timingLimits());
  }

  shouldSuppressDuplicatePartialInternal(text: string): boolean {
    const suppressed = shouldSuppressDuplicatePartial(this.state, text);
    if (suppressed) {
      this.emitWorkerStatus("duplicate-partial");
    }
    return suppressed;
  }

  shouldSuppressFinalInternal(text: string, options: { forcedFinal?: boolean } = {}): boolean {
    const beforeLate = Number(this.state.lateForcedFinalSuppressed || 0);
    const beforeDup = Number(this.state.duplicateFinalSuppressed || 0);
    const suppressed = shouldSuppressFinal(this.state, text, options);
    if (suppressed) {
      if (Number(this.state.lateForcedFinalSuppressed || 0) > beforeLate) {
        this.emitWorkerStatus("late-forced-final");
      } else if (Number(this.state.duplicateFinalSuppressed || 0) > beforeDup) {
        this.emitWorkerStatus("duplicate-final");
      }
    }
    return suppressed;
  }

  buildUpdatePayloadInternal(payload: Record<string, unknown>): Record<string, unknown> {
    return buildTranscriptUpdatePayload(this.state, payload, this.now());
  }

  buildWorkerPayloadInternal(type: string, extra?: Record<string, unknown>): Record<string, unknown> {
    return buildWorkerPayload({
      state: this.state,
      type,
      extra,
      nowMs: this.now(),
      visibilityState: document.hidden ? "hidden" : "visible",
      browserSessionAgeMs: currentSessionAgeMs(this.state, this.now()),
      wakeLockSupported: hasWakeLockSupport(),
    });
  }

  emitWorkerStatus(reason: string): boolean {
    const socket = this.state.socket;
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      return false;
    }
    try {
      socket.send(
        JSON.stringify(
          this.buildWorkerPayloadInternal("browser_asr_status", {
            reason: String(reason || "").trim() || null,
          })
        )
      );
      return true;
    } catch {
      return false;
    }
  }

  emitHeartbeat(reason: string): void {
    const socket = this.state.socket;
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      return;
    }
    try {
      socket.send(
        JSON.stringify(
          this.buildWorkerPayloadInternal("browser_asr_heartbeat", {
            reason: String(reason || "").trim() || "heartbeat",
          })
        )
      );
    } catch {
      // best effort
    }
  }

  sendUpdateInternal(payload: Record<string, unknown>): boolean {
    const socket = this.state.socket;
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      this.setStatusInternal("waiting-for-websocket");
      return false;
    }
    try {
      socket.send(
        JSON.stringify(this.buildWorkerPayloadInternal("external_asr_update", this.buildUpdatePayloadInternal(payload)))
      );
      this.state.appSendCount = Number(this.state.appSendCount || 0) + 1;
      this.updateCountersInternal();
      return true;
    } catch {
      return false;
    }
  }

  scheduleForceFinalizeInternal(): void {
    this.clearForceFinalizeTimerInternal();
    if (!this.isForceFinalizationEnabled() || !this.state.currentPartial) {
      return;
    }
    this.state.forceFinalizeTimer = window.setTimeout(() => {
      if (!this.state.currentPartial || !this.state.desiredRunning) {
        return;
      }
      const finalText = this.state.currentPartial;
      const clientSegmentId = this.ensureClientSegmentIdInternal();
      if (this.shouldSuppressFinalInternal(finalText, { forcedFinal: true })) {
        this.state.currentPartial = "";
        this.options.setPartialText?.("");
        return;
      }
      this.state.missingFinalCount = Number(this.state.missingFinalCount || 0) + 1;
      this.state.forcedCount = Number(this.state.forcedCount || 0) + 1;
      this.sendUpdateInternal({
        partial: finalText,
        final: finalText,
        is_final: true,
        source_lang: this.state.sourceLang,
        client_segment_id: clientSegmentId,
        forced_final: true,
      });
      this.state.currentSegmentLastFinalText = this.normalizeTranscriptTextInternal(finalText);
      this.state.currentSegmentForcedFinalized = true;
      this.state.lastForcedFinal = {
        generation_id: this.currentGenerationIdInternal(),
        client_segment_id: clientSegmentId,
        text: finalText,
        at_ms: this.now(),
      };
      this.state.currentPartial = "";
      this.options.setFinalText?.(finalText);
      this.options.setPartialText?.("");
      this.setStatusInternal("forced-finalized");
      if (recognitionOverlapActive(this.state)) {
        prestartOverlapBuddyIfNeeded(this, overlapActiveSlotIndex(this.state));
      }
      this.updateCountersInternal();
    }, Number(this.state.forceFinalizationTimeoutMs || 1600));
  }

  clearForceFinalizeTimerInternal(): void {
    if (this.state.forceFinalizeTimer) {
      clearTimeout(this.state.forceFinalizeTimer);
      this.state.forceFinalizeTimer = null;
    }
  }

  clearRestartTimerInternal(): void {
    if (this.state.restartTimer) {
      clearTimeout(this.state.restartTimer);
      this.state.restartTimer = null;
    }
  }

  clearReconnectTimerInternal(): void {
    if (this.state.reconnectTimer) {
      clearTimeout(this.state.reconnectTimer);
      this.state.reconnectTimer = null;
    }
  }

  clearAllTimersInternal(): void {
    this.clearForceFinalizeTimerInternal();
    this.clearRestartTimerInternal();
    this.clearReconnectTimerInternal();
    clearOverlapTimeBasedPrestart(this.state);
  }

  recognitionStartBurstThrottleInternal(reason: string) {
    return recognitionStartBurstThrottle(reason, this.recognitionStartLogMinGapMs);
  }

  minimumReconnectGuardDelayMsInternal(delayMs: number): number {
    return minimumReconnectGuardDelayMs(this.state, delayMs, this.now(), this.minimumReconnectIntervalMs);
  }

  canForceFinalizeOnInterruptionInternal(): boolean {
    return canForceFinalizeOnInterruption(this.state, this.isForceFinalizationEnabled());
  }

  forceFinalizeOnInterruptionInternal(reason: string): boolean {
    if (!this.canForceFinalizeOnInterruptionInternal()) {
      return false;
    }
    const finalText = this.normalizeTranscriptTextInternal(this.state.currentPartial);
    const clientSegmentId = this.state.currentClientSegmentId || this.ensureClientSegmentIdInternal();
    if (this.shouldSuppressFinalInternal(finalText, { forcedFinal: true })) {
      return false;
    }
    this.clearForceFinalizeTimerInternal();
    this.state.missingFinalCount = Number(this.state.missingFinalCount || 0) + 1;
    this.state.forcedCount = Number(this.state.forcedCount || 0) + 1;
    this.state.browserForcedFinalOnInterruptionCount =
      Number(this.state.browserForcedFinalOnInterruptionCount || 0) + 1;
    this.state.currentSegmentLastFinalText = finalText;
    this.state.currentSegmentForcedFinalized = true;
    this.state.lastForcedFinal = {
      generation_id: this.currentGenerationIdInternal(),
      client_segment_id: clientSegmentId,
      text: finalText,
      reason: String(reason || "browser_recognition_interrupted"),
      at_ms: this.now(),
    };
    this.state.currentPartial = "";
    this.state.currentPartialStableSinceMs = 0;
    this.options.setFinalText?.(finalText);
    this.options.setPartialText?.("");
    this.sendUpdateInternal({
      partial: finalText,
      final: finalText,
      is_final: true,
      source_lang: this.state.sourceLang,
      client_segment_id: clientSegmentId,
      forced_final: true,
      forced_final_reason: String(reason || "browser_recognition_interrupted"),
    });
    this.setStatusInternal("forced-finalized");
    this.updateCountersInternal();
    return true;
  }

  refreshHealthSignalsInternal(): void {
    const reason = computeHealthDegradedReason({
      state: this.state,
      nowMs: this.now(),
      documentHidden: Boolean(document.hidden),
      limits: this.timingLimits(),
    });
    this.setHealthDegradedReasonInternal(reason || null);
  }

  stripChromeOnDeviceHints(recognition: WorkerSpeechRecognition): void {
    webSpeechRecognitionPolicy.stripChromeOnDeviceHints(recognition);
  }

  applyChromeCompatHintsToRecognition(recognition: WorkerSpeechRecognition): void {
    if (!recognition || !this.state.webSpeechPhraseHintsSuppressed) {
      return;
    }
    this.stripChromeOnDeviceHints(recognition);
  }

  wireRecognitionHandlers(
    recognition: WorkerSpeechRecognition,
    generationId: number,
    overlapSlotIndex: number | null
  ): void {
    wireRecognitionHandlers(this, recognition, generationId, overlapSlotIndex);
  }

  isActiveGeneration(generationId: number): boolean {
    return generationId === Number(this.state.recognitionGenerationId || 0);
  }

  cleanupRecognitionInstance(generationId: number): void {
    cleanupRecognitionInstance(this, generationId);
  }

  recognitionOverlapActiveInternal(): boolean {
    return recognitionOverlapActive(this.state);
  }

  stopInternal(): void {
    this.stop();
  }

  scheduleRestartInternal(reason: string, options?: { backoffMs?: number }): void {
    scheduleRestart(this, reason, options);
  }

  performControlledStartInternal(reason: string): void {
    performControlledStart(this, reason);
  }

  transitionToStoppingInternal(reason: string): void {
    transitionToStopping(this, reason);
  }

  ensureSocketConnectedInternal(): void {
    ensureSocketConnected(this);
  }

  handleSocketMessageInternal(raw: string): void {
    const control = parseBrowserAsrControlMessage(raw);
    if (!control) {
      return;
    }
    if (control.action === "stop") {
      this.stop();
      return;
    }
    if (control.action === "reload_settings") {
      void this.reloadSettingsFromBackend();
    }
  }

  applyRecognitionSettings(): void {
    const settings = this.getRecognitionSettings();
    this.state.configuredLanguage = settings.language || this.state.configuredLanguage || "ru-RU";
    this.state.sourceLang = String(this.state.configuredLanguage.split("-", 1)[0] || "ru").toLowerCase();
    this.state.providerName = String(settings.providerName || this.state.browserMode || "browser_google");
    this.state.actualContinuous = settings.continuous !== false;
    this.state.effectiveContinuousMode = this.state.actualContinuous ? "native_continuous" : "segmented_restart";
    this.state.minimumReconnectIntervalMs = Math.max(
      100,
      Number(settings.minimumReconnectIntervalMs || this.state.minimumReconnectIntervalMs || 500)
    );
    this.state.normalRestartDelayMs = Math.max(0, Number(settings.normalRestartDelayMs || this.state.normalRestartDelayMs || 350));
    this.state.noSpeechRestartDelayMs = Math.max(
      0,
      Number(settings.noSpeechRestartDelayMs || this.state.noSpeechRestartDelayMs || 350)
    );
    this.state.networkReconnectInitialMs = Math.max(
      100,
      Number(settings.networkReconnectInitialMs || this.state.networkReconnectInitialMs || 1000)
    );
    this.state.networkReconnectMaxMs = Math.max(
      this.state.networkReconnectInitialMs,
      Number(settings.networkReconnectMaxMs || this.state.networkReconnectMaxMs || 30000)
    );
    this.state.maxBrowserSessionAgeMs = Math.max(
      10000,
      Number(settings.maxBrowserSessionAgeMs || this.state.maxBrowserSessionAgeMs || 180000)
    );
    this.state.prepareCycleBeforeMs = Math.max(
      0,
      Number(settings.prepareCycleBeforeMs || this.state.prepareCycleBeforeMs || 15000)
    );
    this.state.forceFinalOnInterruption = settings.forceFinalOnInterruption !== false;
    this.state.forceFinalMinChars = Math.max(1, Number(settings.forceFinalMinChars || this.state.forceFinalMinChars || 3));
    this.state.forceFinalMinStableMs = Math.max(
      0,
      Number(settings.forceFinalMinStableMs || this.state.forceFinalMinStableMs || 700)
    );
    this.restartDelayByReasonMs.normal_onend = this.state.normalRestartDelayMs;
    this.restartDelayByReasonMs.settings_change = this.state.normalRestartDelayMs;
    this.restartDelayByReasonMs.websocket_reconnect = this.state.normalRestartDelayMs;
    this.restartDelayByReasonMs.session_cycle = this.state.normalRestartDelayMs;
    this.initialNoSpeechDelayMs = this.state.noSpeechRestartDelayMs;
    this.initialNetworkBackoffMs = this.state.networkReconnectInitialMs;
    this.maxNetworkBackoffMs = this.state.networkReconnectMaxMs;
    this.maxBrowserSessionAgeMs = this.state.maxBrowserSessionAgeMs;
    this.prepareCycleBeforeMs = this.state.prepareCycleBeforeMs;
    this.maxStoppingMs = Math.max(
      500,
      Number(settings.stuckStoppingTimeoutMs || this.state.stuckStoppingTimeoutMs || this.maxStoppingMs)
    );
    this.state.stuckStoppingTimeoutMs = this.maxStoppingMs;
    const targets: WorkerSpeechRecognition[] = [];
    if (this.recognitionOverlapActiveInternal()) {
      (this.state.recognitionOverlapSlots || []).forEach((slot) => {
        if (slot) {
          targets.push(slot);
        }
      });
    } else if (this.state.recognition) {
      targets.push(this.state.recognition);
    }
    if (!targets.length) {
      this.updateCountersInternal();
      return;
    }
    targets.forEach((recognition) => {
      recognition.lang = this.state.configuredLanguage;
      recognition.interimResults = settings.interimResults !== false;
      recognition.continuous = this.state.actualContinuous;
      this.applyChromeCompatHintsToRecognition(recognition);
    });
    this.updateCountersInternal();
  }

  maybeRestartAfterSettingsChange(reason = "settings_change"): void {
    if (!this.state.desiredRunning) {
      return;
    }
    this.appendLogInternal("worker settings changed; controlled restart requested");
    this.state.pendingStart = true;
    this.state.pendingRestartReason = String(reason || "settings_change");
    this.transitionToStoppingInternal("settings-change");
  }

  async reloadSettingsFromBackend(): Promise<void> {
    if (typeof this.options.loadBackendSettings !== "function") {
      return;
    }
    await this.options.loadBackendSettings();
    this.emitWorkerStatus("settings-reloaded");
  }

  async start(): Promise<void> {
    if (!this.SpeechRecognitionCtor) {
      this.setStatusInternal("unsupported-browser");
      return;
    }
    this.state.desiredRunning = true;
    this.clearRestartTimerInternal();
    this.ensureSocketConnectedInternal();
    this.startWatchdog();
    if (this.state.browserSupervisorState === "fatal") {
      this.appendLogInternal("start ignored: supervisor is in fatal state");
      return;
    }
    if (this.state.browserSupervisorState === "running" || this.state.browserSupervisorState === "starting") {
      this.appendLogInternal(`duplicate start ignored (${this.state.browserSupervisorState})`);
      return;
    }
    if (this.state.browserSupervisorState === "stopping") {
      this.state.pendingStart = true;
      this.appendLogInternal("recognition.start deferred: recognition is stopping");
      this.emitWorkerStatus("start-deferred");
      return;
    }
    if (this.state.browserSupervisorState === "restarting" || this.state.browserSupervisorState === "backoff") {
      this.state.pendingStart = true;
      this.appendLogInternal("start requested while restart/backoff is already scheduled");
      return;
    }
    try {
      await ensureMicrophonePermission(this);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Microphone permission was denied.";
      this.setLastErrorInternal("not-allowed", message);
      this.state.desiredRunning = false;
      this.setSupervisorStateInternal("fatal");
      this.setStatusInternal(this.translate("browser_asr.mic_error_status", { message }));
      this.setTerminalDegradedReasonInternal("permission_denied");
      this.emitWorkerStatus("microphone-permission-failed");
      return;
    }
    const proceed = await waitUntilDocumentVisibleForRecognition(this);
    if (!proceed || !this.state.desiredRunning) {
      if (!this.state.desiredRunning) {
        this.appendLogInternal("start aborted while waiting for visibility/focus");
      }
      return;
    }
    this.state.pendingStart = false;
    this.setTerminalDegradedReasonInternal(null);
    resetNetworkErrorBurst(this.state);
    void acquireWakeLock(this, "user-start");
    this.state.webSpeechPhraseHintsSuppressed = false;
    this.state.webSpeechLanguageSoftFallbackUsed = false;
    this.performControlledStartInternal("user-start");
  }

  stop(): void {
    this.appendLogInternal("stop requested by user");
    this.state.desiredRunning = false;
    this.state.pendingStart = false;
    this.state.generationId = Number(this.state.generationId || 0) + 1;
    this.state.webSpeechPhraseHintsSuppressed = false;
    this.state.webSpeechLanguageSoftFallbackUsed = false;
    this.state.browserCyclePending = false;
    this.clearAllTimersInternal();
    this.state.currentPartial = "";
    this.state.currentPartialStableSinceMs = 0;
    this.state.stoppingSinceMs = this.now();
    this.state.pendingRestartReason = null;
    this.state.noSpeechBackoffMs = 0;
    this.state.restartBackoffMs = 0;
    this.resetSegmentTrackingInternal();
    resetNetworkErrorBurst(this.state);
    this.setTerminalDegradedReasonInternal(null);
    this.state.socketDegraded = false;
    this.state.visibilityDegraded = false;
    this.refreshDegradedReasonInternal();
    this.options.setPartialText?.("");
    this.transitionToStoppingInternal("user-stop");
    void releaseWakeLock(this, "user-stop");
    this.emitWorkerStatus("user-stop");
  }

  destroy(): void {
    this.stop();
    this.stopWatchdog();
    void releaseWakeLock(this, "destroy");
    this._appendLogThrottleState = null;
    const socket = this.state.socket;
    this.state.socket = null;
    this.state.websocketReady = false;
    if (socket && socket.readyState <= WebSocket.OPEN) {
      try {
        socket.close();
      } catch {
        // best effort
      }
    }
  }

  handleForceFinalizationSettingChange(): void {
    if (!this.isForceFinalizationEnabled()) {
      this.clearForceFinalizeTimerInternal();
      return;
    }
    if (this.state.currentPartial) {
      this.scheduleForceFinalizeInternal();
    }
  }

  handleVisibilityChange(): void {
    this.state.visibilityDegraded = Boolean(document.hidden && this.state.desiredRunning);
    this.refreshDegradedReasonInternal();
    const supervisor = this.state.browserSupervisorState;
    const startupInFlight = supervisor === "starting" || supervisor === "stopping";
    if (!document.hidden && this.state.desiredRunning && supervisor !== "running" && !startupInFlight) {
      this.scheduleRestartInternal("websocket_reconnect");
    }
    if (!document.hidden) {
      this.refreshHealthSignalsInternal();
      if (this.state.desiredRunning && !this.state.wakeLockActive) {
        void acquireWakeLock(this, "visibility-visible");
      }
    }
    this.emitWorkerStatus("visibility");
  }

  ensureSocketConnected(): void {
    this.ensureSocketConnectedInternal();
  }

  private startWatchdog(): void {
    if (this._watchdogTimer) {
      return;
    }
    this._watchdogTimer = window.setInterval(() => this.runWatchdog(), this.watchdogIntervalMs);
  }

  private stopWatchdog(): void {
    if (this._watchdogTimer) {
      clearInterval(this._watchdogTimer);
      this._watchdogTimer = null;
    }
  }

  private runWatchdog(): void {
    if (!this.state.desiredRunning) {
      return;
    }
    const now = this.now();
    this.refreshHealthSignalsInternal();
    if (
      this.state.healthDegradedReason === "web_speech_stalled" &&
      this.state.browserSupervisorState === "running"
    ) {
      this.state.pendingStart = true;
      this.state.pendingRestartReason = "watchdog_stall";
      this.appendLogInternal("watchdog rearm: web speech stalled with active mic");
      this.transitionToStoppingInternal("watchdog-stall-health");
      return;
    }
    const tick = evaluateWatchdogTick({
      state: this.state,
      nowMs: now,
      limits: {
        maxBrowserSessionAgeMs: this.state.maxBrowserSessionAgeMs,
        prepareCycleBeforeMs: this.state.prepareCycleBeforeMs,
        maxStoppingMs: this.maxStoppingMs,
        hiddenIdleRestartMs: this.hiddenIdleRestartMs,
        visibleIdleRestartMs: this.visibleIdleRestartMs,
      },
      documentHidden: document.hidden,
    });
    if (tick.type === "session_cycle") {
      this.state.browserCycleCount = Number(this.state.browserCycleCount || 0) + 1;
      this.state.pendingStart = true;
      this.state.pendingRestartReason = "session_cycle";
      this.appendLogInternal("browser session age limit reached; controlled cycle requested");
      this.transitionToStoppingInternal("session-cycle");
      this.emitWorkerStatus("session-cycle");
      return;
    }
    if (tick.type === "cycle_pending") {
      this.state.browserCyclePending = true;
      this.emitWorkerStatus("cycle-pending");
    }
    if (tick.type === "stopping_timeout") {
      if (this.recognitionOverlapActiveInternal()) {
        (this.state.recognitionOverlapSlots || []).forEach((recognition) => {
          if (recognition) {
            try {
              recognition.abort();
            } catch {
              // best effort
            }
          }
        });
      } else if (this.state.recognition) {
        try {
          this.state.recognition.abort();
        } catch {
          // best effort
        }
      }
      this.cleanupRecognitionInstance(this.state.recognitionGenerationId);
      this.setRecognitionStateInternal("idle");
      this.state.stoppingSinceMs = null;
      if (this.state.desiredRunning || this.state.pendingStart) {
        this.state.pendingRestartReason = "watchdog_stall";
        this.scheduleRestartInternal("watchdog_stall");
      } else {
        this.setSupervisorStateInternal("idle");
      }
      this.emitWorkerStatus("watchdog-stop");
      return;
    }
    if (tick.type === "idle_rearm") {
      this.state.pendingStart = true;
      this.state.pendingRestartReason = "watchdog_stall";
      this.appendLogInternal("watchdog forced rearm");
      this.transitionToStoppingInternal("watchdog");
      return;
    }
    if (recoverGhostOverlapBuddy(this, now)) {
      return;
    }
    if (tick.type === "heartbeat" || tick.type === "cycle_pending") {
      this.emitHeartbeat("watchdog");
    }
  }
}
