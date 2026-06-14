import type { BrowserLifecycleConfig } from "../asr/types";

export const WORKER_SETTINGS_STORAGE_KEY = "sst.browser_worker.settings.v1";

export const CLIENT_LOG_THROTTLE_MS = 3000;
export const MIC_MONITOR_INTERVAL_MS = 250;
export const MIC_ACTIVE_RMS_THRESHOLD = 0.015;
export const MIC_VOICE_RMS_THRESHOLD = 0.025;

export const workerDefaults = Object.freeze({
  recognitionLanguage: "ru-RU",
  interimResults: true,
  continuousResults: true,
  forceFinalizationEnabled: true,
  forceFinalizationTimeoutMs: 1600,
});

export const browserLifecycleDefaults: BrowserLifecycleConfig = Object.freeze({
  minimumReconnectIntervalMs: 500,
  normalRestartDelayMs: 350,
  noSpeechRestartDelayMs: 350,
  networkReconnectInitialMs: 1000,
  networkReconnectMaxMs: 30000,
  stuckStoppingTimeoutMs: 2500,
  maxBrowserSessionAgeMs: 180000,
  prepareCycleBeforeMs: 15000,
  forceFinalOnInterruption: true,
  forceFinalMinChars: 3,
  forceFinalMinStableMs: 700,
  overlapPrestartAfterMs: 8000,
  overlapBuddyGhostTimeoutMs: 6000,
  overlapBuddyGhostActiveMicMs: 3000,
});

export interface ResolvedWorkerSettings {
  recognition_language: string;
  interim_results: boolean;
  continuous_results: boolean;
  force_finalization_enabled: boolean;
  force_finalization_timeout_ms: number;
}
