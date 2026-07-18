import type { DiagnosticsSnapshot, RuntimeStatus } from "./types";
import {
  formatObsCaptionError,
  formatObsOutputMode,
} from "./obs-status-i18n";

export function diagnosticsFromRuntime(runtime: RuntimeStatus): DiagnosticsSnapshot {
  const raw = runtime as Record<string, unknown>;
  const asrRaw = (raw.asr_diagnostics as Record<string, unknown> | undefined) || {};
  const browserWorker =
    (runtime.asr?.diagnostics?.browser_worker as Record<string, unknown> | undefined) ||
    (asrRaw.browser_worker as Record<string, unknown> | undefined);

  const localModule =
    (runtime.asr?.local_module as Record<string, unknown> | undefined) ||
    (asrRaw.local_module as Record<string, unknown> | undefined);

  return {
    asr: {
      ...asrRaw,
      provider: asrRaw.provider || runtime.asr?.active_mode,
      active_mode: runtime.asr?.active_mode || asrRaw.active_mode,
      browser_worker: browserWorker,
      local_module: localModule,
    },
    translation: (raw.translation_diagnostics as Record<string, unknown> | undefined) || {},
    obs: (raw.obs_caption_diagnostics as Record<string, unknown> | undefined) || {},
    subtitle:
      (raw.subtitle_router_counters as Record<string, unknown> | undefined) || {},
    metrics: runtime.metrics || {},
    healthStatus: runtime.running || runtime.is_running ? "ok" : "idle",
  };
}

export function obsStatusMessage(
  obsConfigEnabled: boolean,
  diagnostics: Record<string, unknown> | undefined,
  tr: (key: string, vars?: Record<string, string>) => string,
): string {
  const diag = diagnostics || {};
  if (!obsConfigEnabled || diag.enabled === false) {
    return tr("obs.cc.status.disabled");
  }
  if (diag.connection_state === "connected") {
    return tr("obs.cc.status.connected", {
      mode: formatObsOutputMode(String(diag.output_mode || "disabled"), tr),
    });
  }
  if (diag.connection_state === "auth_failed") {
    const authError = formatObsCaptionError(String(diag.last_error || "auth_failed"), tr);
    return tr("obs.cc.status.error", { error: authError });
  }
  const lastError = String(diag.last_error || "").trim();
  if (lastError) {
    return tr("obs.cc.status.error", { error: formatObsCaptionError(lastError, tr) });
  }
  return tr("obs.cc.status.waiting");
}
