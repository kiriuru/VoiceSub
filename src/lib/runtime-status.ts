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
  obsStatus: "ready" | "disabled" | "error";
  obsLabel: string;
  lastError: string | null;
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
  return {
    phase: resolveRuntimePhase(runtime),
    running: Boolean(runtime.running || runtime.is_running),
    wsConnected,
    workerConnected: Boolean(browserWorker?.worker_connected),
    asrMode: runtime.asr?.active_mode || "browser_google",
    obsStatus: obs.status,
    obsLabel: obs.label,
    lastError: runtime.last_error ? String(runtime.last_error) : null,
  };
}
