import type { BrowserLifecycleConfig } from "../asr/types";
import { apiFetch } from "../loopback-api-client";
import { applyUiThemeFromConfigPayload } from "../ui-theme";
import {
  browserLifecycleDefaults,
  workerDefaults,
  WORKER_SETTINGS_STORAGE_KEY,
  type ResolvedWorkerSettings,
} from "./worker-defaults";

export function cloneConfigPayload(payload: unknown): Record<string, unknown> {
  try {
    return JSON.parse(JSON.stringify(payload || {})) as Record<string, unknown>;
  } catch {
    return {};
  }
}

export function readWorkerSettingsFromLocalStorage(): Partial<ResolvedWorkerSettings> | null {
  try {
    const raw = window.localStorage.getItem(WORKER_SETTINGS_STORAGE_KEY);
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as unknown;
    return parsed && typeof parsed === "object" ? (parsed as Partial<ResolvedWorkerSettings>) : null;
  } catch {
    return null;
  }
}

export function resolveWorkerSettings(
  backendBrowserConfig: Record<string, unknown> | Partial<ResolvedWorkerSettings> | null | undefined,
  storedWorkerSettings: Partial<ResolvedWorkerSettings> | null | undefined
): ResolvedWorkerSettings {
  const backend = backendBrowserConfig && typeof backendBrowserConfig === "object" ? backendBrowserConfig : {};
  const stored = storedWorkerSettings && typeof storedWorkerSettings === "object" ? storedWorkerSettings : {};
  return {
    recognition_language: String(
      backend.recognition_language || stored.recognition_language || workerDefaults.recognitionLanguage
    ),
    interim_results: Object.prototype.hasOwnProperty.call(stored, "interim_results")
      ? stored.interim_results !== false
      : backend.interim_results !== false,
    continuous_results: Object.prototype.hasOwnProperty.call(stored, "continuous_results")
      ? stored.continuous_results !== false
      : backend.continuous_results !== false,
    force_finalization_enabled: Object.prototype.hasOwnProperty.call(stored, "force_finalization_enabled")
      ? stored.force_finalization_enabled !== false
      : backend.force_finalization_enabled !== false,
    force_finalization_timeout_ms: Math.max(
      300,
      Number.parseInt(
        String(
          stored.force_finalization_timeout_ms ||
            backend.force_finalization_timeout_ms ||
            workerDefaults.forceFinalizationTimeoutMs
        ),
        10
      ) || workerDefaults.forceFinalizationTimeoutMs
    ),
  };
}

export function resolveBrowserLifecycleConfig(
  backendBrowserConfig: Record<string, unknown> | null | undefined
): BrowserLifecycleConfig {
  const backend = backendBrowserConfig && typeof backendBrowserConfig === "object" ? backendBrowserConfig : {};
  return {
    minimumReconnectIntervalMs: Math.max(
      100,
      Number.parseInt(String(backend.minimum_reconnect_interval_ms || browserLifecycleDefaults.minimumReconnectIntervalMs), 10) ||
        browserLifecycleDefaults.minimumReconnectIntervalMs
    ),
    normalRestartDelayMs: Math.max(
      0,
      Number.parseInt(String(backend.normal_restart_delay_ms || browserLifecycleDefaults.normalRestartDelayMs), 10) ||
        browserLifecycleDefaults.normalRestartDelayMs
    ),
    noSpeechRestartDelayMs: Math.max(
      0,
      Number.parseInt(String(backend.no_speech_restart_delay_ms || browserLifecycleDefaults.noSpeechRestartDelayMs), 10) ||
        browserLifecycleDefaults.noSpeechRestartDelayMs
    ),
    networkReconnectInitialMs: Math.max(
      100,
      Number.parseInt(String(backend.network_reconnect_initial_ms || browserLifecycleDefaults.networkReconnectInitialMs), 10) ||
        browserLifecycleDefaults.networkReconnectInitialMs
    ),
    networkReconnectMaxMs: Math.max(
      100,
      Number.parseInt(String(backend.network_reconnect_max_ms || browserLifecycleDefaults.networkReconnectMaxMs), 10) ||
        browserLifecycleDefaults.networkReconnectMaxMs
    ),
    stuckStoppingTimeoutMs: Math.max(
      500,
      Number.parseInt(String(backend.stuck_stopping_timeout_ms || browserLifecycleDefaults.stuckStoppingTimeoutMs), 10) ||
        browserLifecycleDefaults.stuckStoppingTimeoutMs
    ),
    maxBrowserSessionAgeMs: Math.max(
      10000,
      Number.parseInt(String(backend.max_browser_session_age_ms || browserLifecycleDefaults.maxBrowserSessionAgeMs), 10) ||
        browserLifecycleDefaults.maxBrowserSessionAgeMs
    ),
    prepareCycleBeforeMs: Math.max(
      0,
      Number.parseInt(String(backend.prepare_cycle_before_ms || browserLifecycleDefaults.prepareCycleBeforeMs), 10) ||
        browserLifecycleDefaults.prepareCycleBeforeMs
    ),
    forceFinalOnInterruption: backend.force_final_on_interruption !== false,
    forceFinalMinChars: Math.max(
      1,
      Number.parseInt(String(backend.force_final_min_chars || browserLifecycleDefaults.forceFinalMinChars), 10) ||
        browserLifecycleDefaults.forceFinalMinChars
    ),
    forceFinalMinStableMs: Math.max(
      0,
      Number.parseInt(String(backend.force_final_min_stable_ms || browserLifecycleDefaults.forceFinalMinStableMs), 10) ||
        browserLifecycleDefaults.forceFinalMinStableMs
    ),
    overlapBuddyGhostTimeoutMs: Math.max(
      2000,
      Number.parseInt(
        String(backend.overlap_buddy_ghost_timeout_ms || browserLifecycleDefaults.overlapBuddyGhostTimeoutMs),
        10
      ) || browserLifecycleDefaults.overlapBuddyGhostTimeoutMs
    ),
    overlapBuddyGhostActiveMicMs: Math.max(
      500,
      Number.parseInt(
        String(backend.overlap_buddy_ghost_active_mic_ms || browserLifecycleDefaults.overlapBuddyGhostActiveMicMs),
        10
      ) || browserLifecycleDefaults.overlapBuddyGhostActiveMicMs
    ),
  };
}

export function applyDashboardPresentationFromConfig(
  configPayload: Record<string, unknown> | null | undefined,
  setLocale: (code: string) => void,
  translate: (key: string) => string
): void {
  const payload = configPayload && typeof configPayload === "object" ? configPayload : {};
  const ui = payload.ui && typeof payload.ui === "object" ? (payload.ui as Record<string, unknown>) : {};
  const lang = String(ui.language || "")
    .trim()
    .toLowerCase();
  const supported = ["en", "ru", "ja", "ko", "zh"];
  if (lang && supported.includes(lang)) {
    setLocale(lang);
  }
  applyUiThemeFromConfigPayload(payload);
  document.title = translate("document.title.worker");
}

export async function buildSettingsSavePayload(
  nextWorkerSettings: ResolvedWorkerSettings,
  currentConfigPayload: Record<string, unknown> | null
): Promise<Record<string, unknown>> {
  let basePayload: Record<string, unknown> | null = null;
  try {
    const response = await apiFetch("/api/settings/load", { headers: { Accept: "application/json" } });
    if (response.ok) {
      const latest = await response.json();
      if (latest && typeof latest === "object" && latest.payload && typeof latest.payload === "object") {
        basePayload = cloneConfigPayload(latest.payload);
      }
    }
  } catch {
    // fall back to local snapshot
  }
  const payload = basePayload || cloneConfigPayload(currentConfigPayload);
  const currentAsr =
    payload.asr && typeof payload.asr === "object" ? (payload.asr as Record<string, unknown>) : {};
  const currentBrowser =
    currentAsr.browser && typeof currentAsr.browser === "object"
      ? (currentAsr.browser as Record<string, unknown>)
      : {};
  payload.asr = {
    ...currentAsr,
    browser: {
      ...currentBrowser,
      recognition_language: String(nextWorkerSettings.recognition_language || workerDefaults.recognitionLanguage),
      interim_results: nextWorkerSettings.interim_results !== false,
      continuous_results: nextWorkerSettings.continuous_results !== false,
      force_finalization_enabled: nextWorkerSettings.force_finalization_enabled !== false,
      force_finalization_timeout_ms: Math.max(
        300,
        Number.parseInt(
          String(nextWorkerSettings.force_finalization_timeout_ms || workerDefaults.forceFinalizationTimeoutMs),
          10
        ) || workerDefaults.forceFinalizationTimeoutMs
      ),
    },
  };
  return payload;
}
