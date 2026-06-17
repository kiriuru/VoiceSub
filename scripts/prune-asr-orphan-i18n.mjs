#!/usr/bin/env node
/** Remove orphaned ASR/worker i18n keys from shipped dashboard locales. */
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const LOCALES_DIR = path.join(ROOT, "src/lib/i18n/locales");
const LOCALE_FILES = ["en.json", "ru.json", "ja.json", "ko.json", "zh.json"];

const ORPHAN_KEYS = [
  "runtime.start.preparing_experimental",
  "runtime.start.preparing_web_speech",
  "runtime.start.preparing_asr",
  "runtime.local_realtime.line",
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
  "overview.recognition.mode.browser_google_experimental",
  "overview.recognition.mode.local",
  "overview.recognition.provider",
  "overview.recognition.provider.hint",
  "overview.recognition.provider.parakeet",
  "overview.recognition.provider.parakeet_low_latency",
  "overview.recognition.worker_browser.web_hint",
  "worker.description",
  "worker.final.eyebrow",
  "worker.partial.eyebrow",
  "worker.status.eyebrow",
  "worker.settings.eyebrow",
  "worker.settings.default_status",
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
];

let totalRemoved = 0;
for (const file of LOCALE_FILES) {
  const filePath = path.join(LOCALES_DIR, file);
  const data = JSON.parse(fs.readFileSync(filePath, "utf8"));
  let removed = 0;
  for (const key of ORPHAN_KEYS) {
    if (Object.prototype.hasOwnProperty.call(data, key)) {
      delete data[key];
      removed += 1;
    }
  }
  fs.writeFileSync(filePath, `${JSON.stringify(data, null, 2)}\n`, "utf8");
  totalRemoved += removed;
  console.log(`${file}: removed ${removed} keys`);
}
console.log(`Total removed: ${totalRemoved}`);
