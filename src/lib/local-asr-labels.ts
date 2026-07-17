type TrFn = (key: string, vars?: Record<string, string | number>) => string;

export interface LocalModuleBadgeSnapshot {
  ready: boolean;
  phase: string;
  executionProvider: string;
  activeModelFamily: string;
  activeModelVariant: string;
}

function readString(raw: Record<string, unknown>, camel: string, snake: string): string {
  const value = raw[camel] ?? raw[snake];
  return value == null ? "" : String(value);
}

export function readLocalModuleBadgeSnapshot(
  raw: Record<string, unknown> | null | undefined,
): LocalModuleBadgeSnapshot | null {
  if (!raw) return null;
  const configured = readString(raw, "executionProvider", "execution_provider");
  const active = readString(raw, "activeExecutionProvider", "active_execution_provider");
  const executionProvider = (active || configured || "cpu").trim().toLowerCase();
  return {
    ready: Boolean(raw.ready),
    phase: readString(raw, "phase", "phase"),
    executionProvider: executionProvider || "cpu",
    activeModelFamily: readString(raw, "activeModelFamily", "active_model_family") || "parakeet_tdt",
    activeModelVariant: readString(raw, "activeModelVariant", "active_model_variant") || "int8",
  };
}

export function mergeLocalModuleBadgeSnapshots(
  runtime: LocalModuleBadgeSnapshot | null,
  api: LocalModuleBadgeSnapshot | null,
): LocalModuleBadgeSnapshot | null {
  if (!runtime && !api) return null;
  if (!runtime) return api;
  if (!api) return runtime;
  const executionProvider =
    runtime.executionProvider !== "cpu"
      ? runtime.executionProvider
      : api.executionProvider;
  return {
    ready: runtime.ready || api.ready,
    phase: runtime.phase || api.phase,
    executionProvider,
    activeModelFamily: api.activeModelFamily || runtime.activeModelFamily,
    activeModelVariant: api.activeModelVariant || runtime.activeModelVariant,
  };
}

export function formatLocalAsrExecutionProvider(provider: unknown): string {
  const raw = String(provider ?? "cpu").trim().toLowerCase();
  return raw === "cuda" ? "CUDA" : "CPU";
}

export function isLocalAsrCudaProvider(provider: unknown): boolean {
  return String(provider ?? "cpu").trim().toLowerCase() === "cuda";
}

export function formatLocalAsrModelLabel(
  tr: TrFn,
  family?: string | null,
  variant?: string | null,
): string {
  const familyRaw = String(family ?? "parakeet_tdt").trim() || "parakeet_tdt";
  const variantRaw = String(variant ?? "int8").trim() || "int8";
  const variantKey = `local_asr.model.variant.${variantRaw}`;
  const variantLabel = tr(variantKey);
  if (variantLabel !== variantKey) {
    return variantLabel;
  }
  const familyKey = `local_asr.model.family.${familyRaw}`;
  const familyLabel = tr(familyKey);
  if (familyLabel !== familyKey) {
    return `${familyLabel} · ${variantRaw}`;
  }
  return `${familyRaw} · ${variantRaw}`;
}
