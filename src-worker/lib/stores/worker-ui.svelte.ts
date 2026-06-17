import { getLocale, setLocale as setAppLocale, t } from "../../../src/lib/i18n/index";
import type { LocaleCode } from "../../../src/lib/types";

export function createWorkerUiStore() {
  let currentStatusRaw = $state("idle");
  let workerStatus = $state("idle");
  let onSound = $state(false);
  let websocketReady = $state(false);
  let configuredLanguage = $state("loading...");
  let sourceLang = $state("");
  let partialText = $state("");
  let finalText = $state("");
  let interimResults = $state(true);
  let continuousResults = $state(true);
  let forceFinalization = $state(true);
  let forceFinalizationTimeoutMs = $state(1600);
  let settingsSaveStatus = $state("");
  let settingsSaveIsError = $state(false);
  let saveDisabled = $state(false);
  let visibilityWarning = $state("");
  let documentHidden = $state(false);
  let lastConfiguredLanguage = $state("");
  let lastSourceLang = $state("");
  let appVersion = $state("0.5.3");

  function recognitionStatusLabel(value: string): string {
    const normalized = String(value || "").replace(/-/g, "_");
    const key = `worker.recognition.status.${normalized}`;
    const translated = t(key);
    return translated !== key ? translated : String(value || "");
  }

  function setLocale(code: string): void {
    const normalized = code.trim().toLowerCase() as LocaleCode;
    if (["en", "ru", "ja", "ko", "zh"].includes(normalized)) {
      setAppLocale(normalized);
    }
  }

  function updateVisibilityWarning(): void {
    documentHidden = document.hidden;
    visibilityWarning = document.hidden ? t("worker.visibility.hidden_warning") : t("worker.warning.body");
  }

  function updateCountersFromState(state: {
    onSound: boolean;
    websocketReady: boolean;
    configuredLanguage: string;
    sourceLang: string;
  }): void {
    onSound = state.onSound;
    websocketReady = state.websocketReady;
    lastConfiguredLanguage = state.configuredLanguage;
    lastSourceLang = state.sourceLang;
    configuredLanguage = t("worker.counters.language_line", {
      configured: state.configuredLanguage,
      source: state.sourceLang,
    });
    sourceLang = state.sourceLang;
  }

  function onLocaleChanged(): void {
    workerStatus = recognitionStatusLabel(currentStatusRaw);
    updateVisibilityWarning();
    document.title = t("document.title.worker");
    if (lastConfiguredLanguage) {
      configuredLanguage = t("worker.counters.language_line", {
        configured: lastConfiguredLanguage,
        source: lastSourceLang,
      });
    }
  }

  return {
    get workerStatus() {
      return workerStatus;
    },
    get onSound() {
      return onSound;
    },
    get onSoundLabel() {
      return onSound ? t("common.yes") : t("common.no");
    },
    get websocketReady() {
      return websocketReady;
    },
    get socketStatus() {
      return websocketReady ? t("common.connected") : t("common.disconnected");
    },
    get documentHidden() {
      return documentHidden;
    },
    get configuredLanguage() {
      return configuredLanguage;
    },
    get partialText() {
      return partialText;
    },
    get finalText() {
      return finalText;
    },
    get interimResults() {
      return interimResults;
    },
    set interimResults(value: boolean) {
      interimResults = value;
    },
    get continuousResults() {
      return continuousResults;
    },
    set continuousResults(value: boolean) {
      continuousResults = value;
    },
    get forceFinalization() {
      return forceFinalization;
    },
    set forceFinalization(value: boolean) {
      forceFinalization = value;
    },
    get forceFinalizationTimeoutMs() {
      return forceFinalizationTimeoutMs;
    },
    set forceFinalizationTimeoutMs(value: number) {
      forceFinalizationTimeoutMs = value;
    },
    get settingsSaveStatus() {
      return settingsSaveStatus;
    },
    get settingsSaveIsError() {
      return settingsSaveIsError;
    },
    get saveDisabled() {
      return saveDisabled;
    },
    get visibilityWarning() {
      return visibilityWarning;
    },
    get appVersion() {
      return appVersion;
    },
    setAppVersion(value: string) {
      const next = value.trim();
      if (next) appVersion = next;
    },
    get forceFinalizationTimeoutDisabled() {
      return !forceFinalization;
    },
    tr: t,
    getLocale,
    setLocale,
    onLocaleChanged,
    setStatus(value: string) {
      currentStatusRaw = value;
      workerStatus = recognitionStatusLabel(value);
    },
    setPartialText(value: string) {
      partialText = value;
    },
    setFinalText(value: string) {
      finalText = value;
    },
    setSettingsSaveStatus(message: string, isError = false) {
      settingsSaveStatus = message;
      settingsSaveIsError = isError;
    },
    setSaveDisabled(disabled: boolean) {
      saveDisabled = disabled;
    },
    applyResolvedSettings(settings: {
      recognition_language: string;
      interim_results: boolean;
      continuous_results: boolean;
      force_finalization_enabled: boolean;
      force_finalization_timeout_ms: number;
    }) {
      interimResults = settings.interim_results !== false;
      continuousResults = settings.continuous_results !== false;
      forceFinalization = settings.force_finalization_enabled !== false;
      forceFinalizationTimeoutMs = Math.max(300, settings.force_finalization_timeout_ms);
    },
    updateVisibilityWarning,
    updateCountersFromState,
  };
}

export type WorkerUiStore = ReturnType<typeof createWorkerUiStore>;
