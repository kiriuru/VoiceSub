import type { ConfigPayload, TranslationLine } from "./types";
import { ASR_MODE_BROWSER, normalizeAsrMode } from "./asr-mode";
import { normalizeTranslationProviderSettings } from "./translation-provider-settings";
import { PROVIDERS } from "./constants";
import { WEBSPEECH_BROWSER_ADVANCED_DEFAULTS as browserAdv } from "./webspeech-advanced-defaults";

const CANONICAL_TRANSLATION_SLOTS = [
  "translation_1",
  "translation_2",
  "translation_3",
  "translation_4",
  "translation_5",
] as const;

function normalizeTranslationProvider(value: string | undefined, fallback: string): string {
  const provider = String(value || fallback).trim();
  return provider in PROVIDERS ? provider : fallback;
}

function normalizeTranslationLines(
  lines: TranslationLine[] | undefined,
  fallbackProvider: string,
  targetLanguages: string[] | undefined,
): TranslationLine[] {
  const normalized: TranslationLine[] = [];
  if (Array.isArray(lines) && lines.length) {
    lines.forEach((rawLine, index) => {
      if (!rawLine || typeof rawLine !== "object") return;
      let slotId = String(rawLine.slot_id || "").trim().toLowerCase();
      if (!CANONICAL_TRANSLATION_SLOTS.includes(slotId as (typeof CANONICAL_TRANSLATION_SLOTS)[number])) {
        slotId = CANONICAL_TRANSLATION_SLOTS[index] || "";
      }
      const targetLang = String(rawLine.target_lang || "").trim().toLowerCase();
      if (!slotId || !targetLang) return;
      normalized.push({
        slot_id: slotId,
        enabled: rawLine.enabled !== false,
        target_lang: targetLang,
        provider: normalizeTranslationProvider(rawLine.provider, fallbackProvider),
        label: String(rawLine.label || "").trim() || targetLang.toUpperCase(),
      });
    });
  }
  if (!normalized.length) {
    const fallbackTargets =
      Array.isArray(targetLanguages) && targetLanguages.length ? targetLanguages : ["en"];
    fallbackTargets.slice(0, CANONICAL_TRANSLATION_SLOTS.length).forEach((targetLang, index) => {
      const slotId = CANONICAL_TRANSLATION_SLOTS[index];
      if (!slotId) return;
      const lang = String(targetLang || "").trim().toLowerCase() || "en";
      normalized.push({
        slot_id: slotId,
        enabled: true,
        target_lang: lang,
        provider: fallbackProvider,
        label: lang.toUpperCase(),
      });
    });
  }
  return normalized.slice(0, CANONICAL_TRANSLATION_SLOTS.length);
}

function buildCompatTargetLanguages(lines: TranslationLine[]): string[] {
  const seen = new Set<string>();
  return lines
    .filter((line) => line.enabled !== false)
    .map((line) => String(line.target_lang || "").trim().toLowerCase())
    .filter((targetLang) => {
      if (!targetLang || seen.has(targetLang)) return false;
      seen.add(targetLang);
      return true;
    });
}

function intOr(value: unknown, fallback: number): number {
  const n = Number(value);
  return Number.isFinite(n) ? n : fallback;
}

export function normalizeConfigPayload(raw: ConfigPayload): ConfigPayload {
  const config = structuredClone(raw);

  if (!config.translation) config.translation = {};
  if (!Array.isArray(config.translation.lines)) {
    config.translation.lines = config.translation.lines ? [config.translation.lines as never] : [];
  }
  const translation = config.translation;
  const providerFallback =
    String(translation.provider || "google_translate_v2").trim() === "mymemory"
      ? "google_translate_v2"
      : String(translation.provider || "google_translate_v2").trim() || "google_translate_v2";
  translation.provider = providerFallback;
  translation.lines = normalizeTranslationLines(
    translation.lines,
    providerFallback,
    translation.target_languages,
  );
  translation.target_languages = buildCompatTargetLanguages(translation.lines);
  translation.timeout_ms = Math.max(1000, Math.min(60_000, intOr(translation.timeout_ms, 10_000)));
  translation.queue_max_size = Math.max(1, Math.min(64, intOr(translation.queue_max_size, 8)));
  translation.max_concurrent_jobs = Math.max(1, Math.min(8, intOr(translation.max_concurrent_jobs, 2)));
  if (!translation.cache || typeof translation.cache !== "object") {
    translation.cache = {};
  }
  const cache = translation.cache;
  if (cache.enabled === undefined) cache.enabled = true;
  if (cache.persist === undefined) cache.persist = true;
  cache.max_entries = Math.max(0, Math.min(50_000, intOr(cache.max_entries, 5000)));
  if (!translation.provider_limits || typeof translation.provider_limits !== "object") {
    translation.provider_limits = {};
  }
  translation.provider_settings = normalizeTranslationProviderSettings(
    translation.provider_settings as Record<string, unknown> | undefined,
  );
  if (!config.source_lang || typeof config.source_lang !== "string") {
    config.source_lang = "auto";
  } else {
    config.source_lang = config.source_lang.trim().toLowerCase() || "auto";
  }
  if (!config.subtitle_output) config.subtitle_output = {};
  const output = config.subtitle_output;
  if (output.show_source === undefined) output.show_source = true;
  if (output.show_translations === undefined) output.show_translations = true;
  output.max_translation_languages = Math.max(
    0,
    Math.min(5, intOr(output.max_translation_languages, 2)),
  );
  if (!Array.isArray(output.display_order)) {
    output.display_order = ["source", "translation_1"];
  }

  if (!config.asr) config.asr = { mode: ASR_MODE_BROWSER };
  const asr = config.asr as Record<string, unknown>;
  asr.mode = normalizeAsrMode(asr.mode);
  if (!asr.browser || typeof asr.browser !== "object") asr.browser = {};
  const browser = asr.browser as Record<string, unknown>;
  browser.recognition_language =
    String(browser.recognition_language || "ru-RU").trim() || "ru-RU";
  let launchBrowser = String(browser.worker_launch_browser || "auto").trim().toLowerCase();
  if (launchBrowser === "chromium") {
    launchBrowser = "auto";
  }
  browser.worker_launch_browser = ["auto", "google_chrome"].includes(launchBrowser)
    ? launchBrowser
    : "auto";
  if (browser.interim_results === undefined) browser.interim_results = true;
  if (browser.continuous_results === undefined) browser.continuous_results = true;
  if (browser.force_finalization_enabled === undefined) browser.force_finalization_enabled = true;
  browser.force_finalization_timeout_ms = Math.max(
    300,
    Math.min(15_000, intOr(browser.force_finalization_timeout_ms, 1600)),
  );
  const clampBrowserInt = (key: string, minimum: number, maximum: number, fallback: number) =>
    Math.max(minimum, Math.min(maximum, intOr(browser[key], fallback)));
  browser.minimum_reconnect_interval_ms = clampBrowserInt(
    "minimum_reconnect_interval_ms",
    100,
    60_000,
    browserAdv.minimum_reconnect_interval_ms,
  );
  browser.normal_restart_delay_ms = clampBrowserInt(
    "normal_restart_delay_ms",
    0,
    60_000,
    browserAdv.normal_restart_delay_ms,
  );
  browser.no_speech_restart_delay_ms = clampBrowserInt(
    "no_speech_restart_delay_ms",
    0,
    60_000,
    browserAdv.no_speech_restart_delay_ms,
  );
  browser.network_reconnect_initial_ms = clampBrowserInt(
    "network_reconnect_initial_ms",
    100,
    120_000,
    browserAdv.network_reconnect_initial_ms,
  );
  browser.network_reconnect_max_ms = clampBrowserInt(
    "network_reconnect_max_ms",
    100,
    300_000,
    browserAdv.network_reconnect_max_ms,
  );
  browser.stuck_stopping_timeout_ms = clampBrowserInt(
    "stuck_stopping_timeout_ms",
    500,
    30_000,
    browserAdv.stuck_stopping_timeout_ms,
  );
  browser.max_browser_session_age_ms = clampBrowserInt(
    "max_browser_session_age_ms",
    10_000,
    3_600_000,
    browserAdv.max_browser_session_age_ms,
  );
  browser.prepare_cycle_before_ms = clampBrowserInt(
    "prepare_cycle_before_ms",
    0,
    600_000,
    browserAdv.prepare_cycle_before_ms,
  );
  if (browser.force_final_on_interruption === undefined) browser.force_final_on_interruption = true;
  browser.force_final_min_chars = clampBrowserInt(
    "force_final_min_chars",
    1,
    256,
    browserAdv.force_final_min_chars,
  );
  browser.force_final_min_stable_ms = clampBrowserInt(
    "force_final_min_stable_ms",
    0,
    60_000,
    browserAdv.force_final_min_stable_ms,
  );
  delete browser.worker_ui;

  if (!asr.realtime || typeof asr.realtime !== "object") asr.realtime = {};
  const realtime = asr.realtime as Record<string, unknown>;
  const emitMode = String(realtime.partial_emit_mode || "word_growth").toLowerCase();
  realtime.partial_emit_mode = emitMode === "char_delta" ? "char_delta" : "word_growth";
  realtime.partial_min_new_words = Math.max(1, Math.min(32, intOr(realtime.partial_min_new_words, 1)));
  realtime.partial_min_delta_chars = Math.max(0, Math.min(256, intOr(realtime.partial_min_delta_chars, 0)));
  realtime.partial_coalescing_ms = Math.max(0, Math.min(10_000, intOr(realtime.partial_coalescing_ms, 0)));
  // Deprecated legacy keys (kept in sync for old configs; no runtime effect):
  // pause_to_finalize_ms ↔ finalization_hold_ms — use asr.browser.force_finalization_timeout_ms instead.
  // hard_max_phrase_ms ↔ max_segment_ms — unused; normalized for backward compatibility only.
  realtime.max_segment_ms = Math.max(1000, intOr(realtime.max_segment_ms, 5500));

  if (!config.subtitle_lifecycle) config.subtitle_lifecycle = {};
  const lifecycle = config.subtitle_lifecycle as Record<string, unknown>;
  const blockTtl = Math.max(500, intOr(lifecycle.completed_block_ttl_ms, 4500));
  lifecycle.completed_source_ttl_ms = Math.max(
    500,
    intOr(lifecycle.completed_source_ttl_ms, blockTtl),
  );
  lifecycle.completed_translation_ttl_ms = Math.max(
    500,
    intOr(lifecycle.completed_translation_ttl_ms, blockTtl),
  );
  lifecycle.completed_block_ttl_ms = Math.max(
    Number(lifecycle.completed_source_ttl_ms),
    Number(lifecycle.completed_translation_ttl_ms),
  );
  // @deprecated — see comment above on asr.realtime.max_segment_ms
  lifecycle.pause_to_finalize_ms = Math.max(
    120,
    intOr(lifecycle.pause_to_finalize_ms, intOr(realtime.finalization_hold_ms, 350)),
  );
  realtime.finalization_hold_ms = lifecycle.pause_to_finalize_ms;
  lifecycle.allow_early_replace_on_next_final =
    lifecycle.allow_early_replace_on_next_final !== false;
  lifecycle.sync_source_and_translation_expiry =
    lifecycle.sync_source_and_translation_expiry !== false;
  lifecycle.keep_completed_translation_during_active_partial =
    lifecycle.keep_completed_translation_during_active_partial !== false;
  // @deprecated — see comment above on asr.realtime.max_segment_ms
  lifecycle.hard_max_phrase_ms = Math.max(
    1000,
    intOr(lifecycle.hard_max_phrase_ms, intOr(realtime.max_segment_ms, 5500)),
  );
  realtime.max_segment_ms = lifecycle.hard_max_phrase_ms;

  const OBS_OUTPUT_MODES = [
    "disabled",
    "source_live",
    "source_final_only",
    "translation_1",
    "translation_2",
    "translation_3",
    "translation_4",
    "translation_5",
    "first_visible_line",
  ] as const;
  const obs = (config.obs_closed_captions || {}) as Record<string, unknown>;
  const connection = (obs.connection || {}) as Record<string, unknown>;
  const debugMirror = (obs.debug_mirror || {}) as Record<string, unknown>;
  const timing = (obs.timing || {}) as Record<string, unknown>;
  const rawOutputMode = String(obs.output_mode || "disabled");
  const outputMode = OBS_OUTPUT_MODES.includes(
    rawOutputMode as (typeof OBS_OUTPUT_MODES)[number],
  )
    ? rawOutputMode
    : "disabled";
  config.obs_closed_captions = {
    enabled: obs.enabled === true,
    output_mode: outputMode,
    connection: {
      host: String(connection.host || "127.0.0.1").trim() || "127.0.0.1",
      port: Math.max(1, Math.min(65535, intOr(connection.port, 4455))),
      password: String(connection.password || ""),
    },
    debug_mirror: {
      enabled: debugMirror.enabled === true,
      input_name: String(debugMirror.input_name || "CC_DEBUG").trim() || "CC_DEBUG",
      send_partials: debugMirror.send_partials !== false,
    },
    timing: {
      send_partials: timing.send_partials !== false,
      partial_throttle_ms: Math.max(0, intOr(timing.partial_throttle_ms, 140)),
      min_partial_delta_chars: Math.max(0, intOr(timing.min_partial_delta_chars, 1)),
      final_replace_delay_ms: Math.max(0, intOr(timing.final_replace_delay_ms, 0)),
      clear_after_ms: Math.max(0, intOr(timing.clear_after_ms, 2500)),
      avoid_duplicate_text: timing.avoid_duplicate_text !== false,
    },
  };

  const repl = (config.source_text_replacement || {}) as Record<string, unknown>;
  const wholeWords = !(repl.whole_words === false || repl.whole_word_only === false);
  const includeBuiltin =
    repl.include_builtin !== false && repl.include_builtin_profanity !== false;
  config.source_text_replacement = {
    enabled: repl.enabled === true,
    include_builtin: includeBuiltin,
    include_builtin_profanity: includeBuiltin,
    case_insensitive: repl.case_insensitive !== false,
    whole_words: wholeWords,
    whole_word_only: wholeWords,
    pairs: Array.isArray(repl.pairs) ? repl.pairs : [],
  };

  if (!config.overlay || typeof config.overlay !== "object") {
    config.overlay = { preset: "single", compact: false };
  }
  const overlay = config.overlay as Record<string, unknown>;
  let preset = String(overlay.preset || "single").trim();
  let compact = overlay.compact === true;
  if (preset === "compact") {
    compact = true;
    preset = "stacked";
  }
  overlay.preset = ["single", "dual-line", "stacked"].includes(preset) ? preset : "single";
  overlay.compact = compact;

  if (!config.logging || typeof config.logging !== "object") {
    config.logging = {};
  }
  config.logging.full_enabled = config.logging.full_enabled === true;

  if (!config.ui || typeof config.ui !== "object") {
    config.ui = { language: "" };
  }
  const ui = config.ui as Record<string, unknown>;
  const lang = String(ui.language || "").trim().toLowerCase();
  if (!lang) {
    ui.language = "";
  } else if (["en", "ru", "ja", "ko", "zh"].includes(lang)) {
    ui.language = lang;
  } else if (lang.startsWith("ru")) {
    ui.language = "ru";
  } else if (lang.startsWith("zh")) {
    ui.language = "zh";
  } else if (lang.startsWith("ja")) {
    ui.language = "ja";
  } else if (lang.startsWith("ko")) {
    ui.language = "ko";
  } else if (lang.startsWith("en")) {
    ui.language = "en";
  } else {
    ui.language = "";
  }
  if (ui.show_translation_results === undefined) {
    ui.show_translation_results = true;
  } else {
    ui.show_translation_results = ui.show_translation_results === true;
  }

  return config;
}
