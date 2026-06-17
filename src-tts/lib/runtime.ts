import type { RuntimeStatus } from "./types";
import { apiFetch } from "./loopback-api-client";

export async function fetchRuntimeStatus(): Promise<RuntimeStatus> {
  const res = await apiFetch("/api/runtime/status");
  if (!res.ok) {
    throw new Error(`runtime status -> ${res.status}`);
  }
  return res.json() as Promise<RuntimeStatus>;
}

export function isRuntimeActive(runtime: RuntimeStatus | null | undefined): boolean {
  return Boolean(runtime?.running || runtime?.is_running);
}
