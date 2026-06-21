#!/usr/bin/env node
/** Remove orphaned local-Parakeet / SST ASR i18n keys from dashboard locale sources. */
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

const ROOT = path.resolve(import.meta.dirname, "..");
const JSON_LOCALES_DIR = path.join(ROOT, "src/lib/i18n/locales");
const JS_LOCALES_DIR = path.join(ROOT, "scripts/i18n-source/locales");
const LOCALE_CODES = ["en", "ru", "ja", "ko", "zh"];

const PREFIXES = ["tools.advanced.", "tools.safe.", "tuning."];

const EXACT_KEYS = [
  "compact.nav.asr_advanced",
  "compact.nav.tuning",
  "tab.asr_advanced",
  "tab.tuning",
  "runtime.local_realtime.line",
  "diagnostics.local_parakeet.line",
  "help.recognition.body",
  "help.recognition.eyebrow",
  "help.recognition.local",
  "help.recognition.mic",
  "overview.recognition.browser_mic_note",
  "overview.recognition.hint.browser_google",
  "overview.recognition.hint.browser_google_experimental",
  "overview.recognition.hint.browser_quick_start_locked",
  "overview.recognition.hint.local",
  "overview.recognition.mode",
  "overview.recognition.mode.local",
  "overview.recognition.mode.browser_google_experimental",
  "overview.recognition.provider",
  "overview.recognition.provider.hint",
  "overview.recognition.provider.parakeet",
  "overview.recognition.provider.parakeet_low_latency",
  "overview.recognition.worker_browser.web_hint",
  "runtime.start.preparing_experimental",
  "runtime.start.preparing_web_speech",
  "runtime.start.preparing_asr",
  "document.title.worker_experimental",
  "worker.experimental.settings_local_only",
  "worker.experimental.description",
  "worker.experimental.settings.eyebrow",
  "worker.experimental.settings.note_body",
  "worker.experimental.settings.note_title",
  "worker.experimental.title",
  "worker.experimental.warning.body",
  "browser_asr.error.no_audio_track",
  "browser_asr.error.wrong_track_kind",
  "browser_asr.error.track_not_live",
  "browser_asr.error.open_mic_track",
  "browser_asr.error.track_recovery_failed",
  "browser_asr.error.fallback_default_start",
  "browser_asr.error.experimental_start_failed",
  "document.title.dashboard",
  "header.description",
  "header.eyebrow",
  "header.title",
  "tools.source_replacement.custom_hint",
  "tools.source_replacement.custom_label",
  "tools.source_replacement.pair_checkbox_aria",
  "tools.source_replacement.replace_placeholder",
  "tools.source_replacement.word_placeholder",
  "worker.description",
  "worker.final.eyebrow",
  "worker.metric.app_sends",
  "worker.metric.approx",
  "worker.metric.final",
  "worker.metric.forced",
  "worker.metric.missing",
  "worker.microphone.detected",
  "worker.microphone.eyebrow",
  "worker.microphone.refresh",
  "worker.microphone.request_access",
  "worker.microphone.status.waiting",
  "worker.open_external_failed_log",
  "worker.partial.eyebrow",
  "worker.settings.default_status",
  "worker.settings.eyebrow",
  "worker.settings.saved_local",
  "worker.status.eyebrow",
];

export function shouldRemoveAsrOrphanKey(key) {
  if (EXACT_KEYS.includes(key)) {
    return true;
  }
  return PREFIXES.some((prefix) => key.startsWith(prefix));
}

function pruneObject(data) {
  let removed = 0;
  for (const key of Object.keys(data)) {
    if (shouldRemoveAsrOrphanKey(key)) {
      delete data[key];
      removed += 1;
    }
  }
  return removed;
}

function serializeLocaleJs(code, localeCode, data) {
  const lines = Object.entries(data).map(
    ([key, value]) => `  ${JSON.stringify(key)}: ${JSON.stringify(value)},`,
  );
  return `(function () {
  window.__SST_I18N_LOCALES = window.__SST_I18N_LOCALES || {};
  window.__SST_I18N_LOCALES.${localeCode} = {
${lines.join("\n")}
  };
})();\n`;
}

function loadLocaleJs(filePath, localeCode) {
  const code = fs.readFileSync(filePath, "utf8");
  const sandbox = { window: { __SST_I18N_LOCALES: {} } };
  vm.runInNewContext(code, sandbox);
  return { code, data: sandbox.window.__SST_I18N_LOCALES[localeCode] || {} };
}

let totalRemoved = 0;

for (const code of LOCALE_CODES) {
  const jsonPath = path.join(JSON_LOCALES_DIR, `${code}.json`);
  if (fs.existsSync(jsonPath)) {
    const data = JSON.parse(fs.readFileSync(jsonPath, "utf8"));
    const removed = pruneObject(data);
    fs.writeFileSync(jsonPath, `${JSON.stringify(data, null, 2)}\n`, "utf8");
    totalRemoved += removed;
    console.log(`${path.basename(jsonPath)}: removed ${removed} keys`);
  }

  const jsPath = path.join(JS_LOCALES_DIR, `${code}.js`);
  if (fs.existsSync(jsPath)) {
    const { data } = loadLocaleJs(jsPath, code);
    const removed = pruneObject(data);
    fs.writeFileSync(jsPath, serializeLocaleJs("", code, data), "utf8");
    totalRemoved += removed;
    console.log(`${path.basename(jsPath)}: removed ${removed} keys`);
  }
}

console.log(`Total removed: ${totalRemoved}`);
