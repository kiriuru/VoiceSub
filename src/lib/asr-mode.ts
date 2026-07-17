export const ASR_MODE_BROWSER = "browser_google";
export const ASR_MODE_LOCAL_PARAKEET = "local_parakeet";

export const ASR_MODES = [ASR_MODE_BROWSER, ASR_MODE_LOCAL_PARAKEET] as const;
export type AsrMode = (typeof ASR_MODES)[number];

export interface LocalAsrModuleSnapshot {
  ready?: boolean;
  cuda_ready?: boolean;
  phase?: string;
  execution_provider?: string;
  active_execution_provider?: string;
  active_model_family?: string;
  active_model_variant?: string;
  message?: string;
}

export function normalizeAsrMode(value: unknown): AsrMode {
  const mode = String(value || ASR_MODE_BROWSER).trim();
  return mode === ASR_MODE_LOCAL_PARAKEET ? ASR_MODE_LOCAL_PARAKEET : ASR_MODE_BROWSER;
}

export function isLocalAsrMode(mode: unknown): boolean {
  return normalizeAsrMode(mode) === ASR_MODE_LOCAL_PARAKEET;
}

export function localAsrReadyFromRuntime(
  runtime: { asr?: { local_module?: LocalAsrModuleSnapshot } } | null | undefined,
): boolean {
  return Boolean(runtime?.asr?.local_module?.ready);
}

export function resolveAsrModeLabelKey(mode: unknown): string {
  return isLocalAsrMode(mode)
    ? "overview.recognition.mode.local_asr"
    : "overview.recognition.mode.browser_google";
}
