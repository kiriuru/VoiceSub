export type ProcessResourceSnapshot = {
  pid: number;
  name: string;
  handle_count: number;
  commit_bytes: number;
  working_set_bytes: number;
};

export type ResourceTelemetry = {
  self_process: ProcessResourceSnapshot;
  watched: ProcessResourceSnapshot[];
};

export function formatCompactBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0";
  if (bytes >= 1_073_741_824) {
    return `${(bytes / 1_073_741_824).toFixed(1)}G`;
  }
  if (bytes >= 1_048_576) {
    return `${Math.round(bytes / 1_048_576)}M`;
  }
  return `${Math.round(bytes / 1024)}K`;
}

export function formatHandleCount(count: number): string {
  if (!Number.isFinite(count) || count <= 0) return "0";
  if (count >= 10_000) {
    return `${(count / 1000).toFixed(1)}k`;
  }
  return String(count);
}

export function isResourceTelemetryWarning(snapshot: ProcessResourceSnapshot): boolean {
  return (
    snapshot.handle_count >= 10_000 ||
    snapshot.commit_bytes >= 3 * 1024 * 1024 * 1024
  );
}

export function findWatchedProcess(
  telemetry: ResourceTelemetry | null,
  executable: string,
): ProcessResourceSnapshot | null {
  if (!telemetry) return null;
  const needle = executable.toLowerCase();
  return (
    telemetry.watched.find((entry) => entry.name.toLowerCase() === needle) ??
    null
  );
}
