import {
  ASR_MODE_BROWSER,
  ASR_MODE_LOCAL_PARAKEET,
  isLocalAsrMode,
  normalizeAsrMode,
} from "./asr-mode";
import type { RuntimeStatus } from "./types";

export type RuntimePhaseChip =
  | "idle"
  | "starting"
  | "listening"
  | "transcribing"
  | "translating"
  | "error";

export const RUNTIME_STATE_PHASES: RuntimePhaseChip[] = [
  "idle",
  "starting",
  "listening",
  "transcribing",
  "translating",
  "error",
];

export interface RuntimeConnectionChips {
  phase: string;
  running: boolean;
  wsConnected: boolean;
  workerConnected: boolean;
  asrMode: string;
  asrModeLabelKey: string;
  asrSourceConnected: boolean;
  showBrowserWorkerChip: boolean;
  showLocalAsrChip: boolean;
  obsStatus: "ready" | "disabled" | "error";
  obsLabel: string;
  lastError: string | null;
  statusMessage: string | null;
}

export function resolveRuntimePhase(runtime: RuntimeStatus): string {
  return String(runtime.phase || runtime.status || "idle");
}

export function resolveObsChipStatus(
  obsDiagnostics: Record<string, unknown> | undefined,
  runtime: RuntimeStatus,
): { status: "ready" | "disabled" | "error"; label: string } {
  const obsDiag =
    obsDiagnostics || (runtime.obs_caption_diagnostics as Record<string, unknown> | undefined) || {};
  const lastError = String(obsDiag.last_error || "").trim();
  if (lastError) {
    return { status: "error", label: lastError };
  }
  if (obsDiag.enabled) {
    return { status: "ready", label: String(obsDiag.output_mode || "ready") };
  }
  return { status: "disabled", label: "disabled" };
}

export function buildRuntimeConnectionChips(
  runtime: RuntimeStatus,
  wsConnected: boolean,
  obsDiagnostics?: Record<string, unknown>,
): RuntimeConnectionChips {
  const browserWorker = runtime.asr?.diagnostics?.browser_worker as Record<string, unknown> | undefined;
  const obs = resolveObsChipStatus(obsDiagnostics, runtime);
  const asrMode = normalizeAsrMode(runtime.asr?.active_mode || ASR_MODE_BROWSER);
  const useLocalAsr = isLocalAsrMode(asrMode);
  const phase = resolveRuntimePhase(runtime);
  const running = Boolean(runtime.running || runtime.is_running);

  const asrSourceConnected = useLocalAsr
    ? running && (phase === "listening" || phase === "transcribing")
    : Boolean(browserWorker?.worker_connected);

  return {
    phase,
    running,
    wsConnected,
    workerConnected: Boolean(browserWorker?.worker_connected),
    asrMode,
    asrModeLabelKey:
      asrMode === ASR_MODE_LOCAL_PARAKEET
        ? "overview.recognition.mode.local_asr"
        : "overview.recognition.mode.browser_google",
    asrSourceConnected,
    showBrowserWorkerChip: !useLocalAsr,
    showLocalAsrChip: useLocalAsr,
    obsStatus: obs.status,
    obsLabel: obs.label,
    lastError: runtime.last_error ? String(runtime.last_error) : null,
    statusMessage: runtime.status_message ? String(runtime.status_message) : null,
  };
}
