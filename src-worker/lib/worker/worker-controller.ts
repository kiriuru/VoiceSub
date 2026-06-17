import { apiFetch } from "../loopback-api-client";
import { getLocale } from "../../../src/lib/i18n/index";
import type { SpeechRecognitionConstructor } from "../asr/speech-types";
import { createBrowserAsrStateSeed } from "../asr/session-state";
import { BrowserAsrSessionManager } from "../asr/session-manager";
import type { BrowserAsrState } from "../asr/types";
import { autoLoadAndApplyUiTheme } from "../ui-theme";
import { subscribeUiConfigSync } from "../../../src/lib/ui-config-sync";
import type { WorkerUiStore } from "../stores/worker-ui.svelte";
import { appendWorkerLog } from "./client-log";
import { ensureMicrophonePermission, releaseMicrophoneMonitor } from "./mic-monitor";
import {
  browserLifecycleDefaults,
  workerDefaults,
  WORKER_SETTINGS_STORAGE_KEY,
  type ResolvedWorkerSettings,
} from "./worker-defaults";
import {
  applyDashboardPresentationFromConfig,
  buildSettingsSavePayload,
  cloneConfigPayload,
  readWorkerSettingsFromLocalStorage,
  resolveBrowserLifecycleConfig,
  resolveWorkerSettings,
} from "./worker-config";

function formatNow(): string {
  return new Date().toLocaleTimeString([], { hour12: false });
}

export interface WorkerControllerActions {
  onStart: () => Promise<void>;
  onStop: () => void;
  onSave: () => Promise<void>;
  onInterimChange: () => Promise<void>;
  onContinuousChange: () => Promise<void>;
  onForceFinalizationChange: () => Promise<void>;
  onForceFinalizationTimeoutChange: () => Promise<void>;
}

export interface WorkerController {
  sessionManager: BrowserAsrSessionManager;
  destroy(): void;
  actions: WorkerControllerActions;
}

export function createWorkerController(ui: WorkerUiStore): WorkerController {
  const SpeechRecognitionCtor = (window.SpeechRecognition ||
    window.webkitSpeechRecognition ||
    null) as SpeechRecognitionConstructor | null;

  const state: BrowserAsrState = createBrowserAsrStateSeed({
    browserMode: "browser_google",
    forceFinalizationTimeoutMs: workerDefaults.forceFinalizationTimeoutMs,
    browserLifecycleConfig: { ...browserLifecycleDefaults },
    settingsSavePromise: Promise.resolve(),
  });

  function appendLog(message: string): void {
    appendWorkerLog(message);
  }

  function readForceFinalizationTimeoutMs(): number {
    const raw = Number.parseInt(String(ui.forceFinalizationTimeoutMs || ""), 10);
    return Math.max(300, Math.min(15000, Number.isFinite(raw) ? raw : state.forceFinalizationTimeoutMs));
  }

  function readWorkerSettingsFromControls(): ResolvedWorkerSettings {
    return {
      recognition_language: String(state.configuredLanguage || workerDefaults.recognitionLanguage),
      interim_results: ui.interimResults,
      continuous_results: ui.continuousResults,
      force_finalization_enabled: ui.forceFinalization,
      force_finalization_timeout_ms: readForceFinalizationTimeoutMs(),
    };
  }

  function updateCounters(): void {
    ui.updateCountersFromState({
      onSound: state.onSound,
      websocketReady: state.websocketReady,
      configuredLanguage: state.configuredLanguage,
      sourceLang: state.sourceLang,
    });
  }

  function applyResolvedWorkerSettings(resolvedWorkerSettings: ResolvedWorkerSettings): ResolvedWorkerSettings {
    const effectiveBrowserConfig = resolveWorkerSettings(resolvedWorkerSettings, null);
    state.configuredLanguage = String(effectiveBrowserConfig.recognition_language || workerDefaults.recognitionLanguage);
    state.sourceLang = String(state.configuredLanguage.split("-", 1)[0] || "ru").toLowerCase();
    ui.applyResolvedSettings(effectiveBrowserConfig);
    state.forceFinalizationTimeoutMs = effectiveBrowserConfig.force_finalization_timeout_ms;
    sessionManager.applyRecognitionSettings();
    sessionManager.handleForceFinalizationSettingChange();
    updateCounters();
    return effectiveBrowserConfig;
  }

  function applyLoadedConfigPayload(
    configPayload: Record<string, unknown> | null | undefined,
    storedWorkerSettings: Partial<ResolvedWorkerSettings> | null = null
  ): ResolvedWorkerSettings {
    const payload = configPayload && typeof configPayload === "object" ? configPayload : {};
    state.currentConfigPayload = cloneConfigPayload(payload);
    const asrConfig = payload.asr && typeof payload.asr === "object" ? (payload.asr as Record<string, unknown>) : {};
    const browserConfig =
      asrConfig.browser && typeof asrConfig.browser === "object" ? (asrConfig.browser as Record<string, unknown>) : {};
    state.browserLifecycleConfig = resolveBrowserLifecycleConfig(browserConfig);
    applyDashboardPresentationFromConfig(payload, ui.setLocale, ui.tr);
    return applyResolvedWorkerSettings(resolveWorkerSettings(browserConfig, storedWorkerSettings));
  }

  async function loadSettings(): Promise<void> {
    const storedWorkerSettings = readWorkerSettingsFromLocalStorage();
    try {
      const response = await apiFetch("/api/settings/load");
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      const data = await response.json();
      const effectiveBrowserConfig = applyLoadedConfigPayload(data?.payload || {}, storedWorkerSettings);
      appendLog(
        `settings loaded: lang=${effectiveBrowserConfig.recognition_language}, interim=${effectiveBrowserConfig.interim_results}, continuous=${effectiveBrowserConfig.continuous_results}, force_finalization=${effectiveBrowserConfig.force_finalization_enabled}, source=${storedWorkerSettings ? "localStorage+backend" : "backend"}`
      );
      updateCounters();
    } catch {
      if (storedWorkerSettings) {
        applyResolvedWorkerSettings(resolveWorkerSettings(workerDefaults as unknown as Record<string, unknown>, storedWorkerSettings));
        state.currentConfigPayload = cloneConfigPayload({ asr: { browser: workerDefaults } });
        state.browserLifecycleConfig = { ...browserLifecycleDefaults };
        appendLog("settings load failed; using localStorage fallback");
      } else {
        applyResolvedWorkerSettings({
          recognition_language: workerDefaults.recognitionLanguage,
          interim_results: workerDefaults.interimResults,
          continuous_results: workerDefaults.continuousResults,
          force_finalization_enabled: workerDefaults.forceFinalizationEnabled,
          force_finalization_timeout_ms: workerDefaults.forceFinalizationTimeoutMs,
        });
        state.currentConfigPayload = cloneConfigPayload({ asr: { browser: workerDefaults } });
        state.browserLifecycleConfig = { ...browserLifecycleDefaults };
        appendLog("settings load failed; using defaults");
      }
      updateCounters();
    }
  }

  async function queueBrowserSettingsSave(
    reason: string,
    options: { restartRecognition?: boolean } = {}
  ): Promise<void> {
    const nextWorkerSettings = readWorkerSettingsFromControls();
    const previousPayload = cloneConfigPayload(state.currentConfigPayload);
    const restartRecognition = options.restartRecognition === true;
    try {
      window.localStorage.setItem(WORKER_SETTINGS_STORAGE_KEY, JSON.stringify(nextWorkerSettings));
    } catch {
      // best effort
    }
    applyResolvedWorkerSettings(nextWorkerSettings);
    if (restartRecognition) {
      sessionManager.maybeRestartAfterSettingsChange("settings_change");
    }
    ui.setSettingsSaveStatus(ui.tr("worker.settings.saving"), false);
    ui.setSaveDisabled(true);
    state.settingsSavePromise = state.settingsSavePromise
      .catch(() => undefined)
      .then(async () => {
        const savePayload = await buildSettingsSavePayload(nextWorkerSettings, state.currentConfigPayload);
        const response = await apiFetch("/api/settings/save", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ payload: savePayload }),
        });
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        const data = await response.json();
        const savedBrowser = applyLoadedConfigPayload(data?.payload || {}, nextWorkerSettings);
        updateCounters();
        appendLog(
          `worker settings saved to localStorage+backend (${reason}): lang=${savedBrowser.recognition_language}, interim=${savedBrowser.interim_results}, continuous=${savedBrowser.continuous_results}, force_finalization=${savedBrowser.force_finalization_enabled}`
        );
        ui.setSettingsSaveStatus(ui.tr("worker.settings.saved_backend", { time: formatNow() }), false);
      })
      .catch((error: unknown) => {
        const message = error instanceof Error ? error.message : String(error || "");
        state.currentConfigPayload = previousPayload;
        appendLog(`worker settings backend mirror failed (${reason}): ${message}`);
        ui.setSettingsSaveStatus(
          ui.tr("worker.settings.saved_local_backend_failed", { message: message || ui.tr("common.unknown") }),
          true
        );
      })
      .finally(() => {
        ui.setSaveDisabled(false);
      });
    return state.settingsSavePromise;
  }

  const sessionManager = new BrowserAsrSessionManager({
    state,
    SpeechRecognitionCtor,
    locale: () => getLocale(),
    translate: (key, vars) => ui.tr(key, vars),
    appendLog,
    setStatus: (status) => ui.setStatus(status),
    updateCounters,
    ensureMicrophonePermission: () => ensureMicrophonePermission(state, appendLog),
    getRecognitionSettings: () => ({
      language: state.configuredLanguage,
      interimResults: ui.interimResults,
      continuous: ui.continuousResults,
      providerName: state.browserMode,
      ...state.browserLifecycleConfig,
    }),
    isForceFinalizationEnabled: () => ui.forceFinalization,
    setPartialText: (value) => ui.setPartialText(value),
    setFinalText: (value) => ui.setFinalText(value),
    loadBackendSettings: loadSettings,
  });

  const requestedLocale = new URLSearchParams(window.location.search).get("locale");
  if (requestedLocale) {
    ui.setLocale(requestedLocale);
  }
  document.title = ui.tr("document.title.worker");
  ui.setStatus("idle");
  updateCounters();
  ui.updateVisibilityWarning();
  appendLog("worker initialized");

  void apiFetch("/api/version")
    .then(async (response) => {
      if (!response.ok) return;
      const body = (await response.json()) as { version?: string };
      if (body.version) ui.setAppVersion(body.version);
    })
    .catch(() => {});

  void autoLoadAndApplyUiTheme();

  const unsubscribeUiConfigSync = subscribeUiConfigSync((payload) => {
    applyDashboardPresentationFromConfig(payload as Record<string, unknown>, ui.setLocale, ui.tr);
    appendLog("ui theme synced from dashboard");
  });

  const onStart = async () => {
    await sessionManager.start();
  };
  const onStop = () => sessionManager.stop();
  const onSave = async () => queueBrowserSettingsSave("manual");
  const onInterimChange = async () => queueBrowserSettingsSave("interim_results", { restartRecognition: true });
  const onContinuousChange = async () => queueBrowserSettingsSave("continuous_results", { restartRecognition: true });
  const onForceFinalizationChange = async () => {
    state.forceFinalizationTimeoutMs = readForceFinalizationTimeoutMs();
    sessionManager.handleForceFinalizationSettingChange();
    await queueBrowserSettingsSave("force_finalization");
  };
  const onForceFinalizationTimeoutChange = async () => {
    state.forceFinalizationTimeoutMs = readForceFinalizationTimeoutMs();
    ui.forceFinalizationTimeoutMs = state.forceFinalizationTimeoutMs;
    sessionManager.handleForceFinalizationSettingChange();
    await queueBrowserSettingsSave("force_finalization_timeout");
  };

  let destroyed = false;
  const destroy = () => {
    if (destroyed) {
      return;
    }
    destroyed = true;
    unsubscribeUiConfigSync();
    sessionManager.destroy();
    releaseMicrophoneMonitor(state);
  };

  const onLocaleChanged = () => {
    ui.onLocaleChanged();
    updateCounters();
  };

  window.addEventListener("beforeunload", destroy);
  window.addEventListener("pagehide", destroy);
  window.addEventListener("sst:locale-changed", onLocaleChanged);
  document.addEventListener("visibilitychange", () => {
    appendLog(`document visibility changed: ${document.hidden ? "hidden" : "visible"}`);
    ui.updateVisibilityWarning();
    sessionManager.handleVisibilityChange();
  });
  window.addEventListener("blur", () => {
    appendLog("window blur");
    sessionManager.handleVisibilityChange();
  });
  window.addEventListener("focus", () => {
    appendLog("window focus");
    sessionManager.handleVisibilityChange();
  });

  void loadSettings().finally(() => {
    sessionManager.ensureSocketConnected();
    updateCounters();
    ui.updateVisibilityWarning();
    const params = new URLSearchParams(location.search);
    appendLog(`worker ready; autostart=${params.get("autostart") === "1" ? "yes" : "no"}`);
    if (params.get("autostart") === "1") {
      window.setTimeout(() => {
        void sessionManager.start();
      }, 150);
    }
  });

  return {
    sessionManager,
    destroy,
    actions: {
      onStart,
      onStop,
      onSave,
      onInterimChange,
      onContinuousChange,
      onForceFinalizationChange,
      onForceFinalizationTimeoutChange,
    },
  };
}
