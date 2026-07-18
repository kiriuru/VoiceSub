import type { ConfigPayload, RuntimeStatus, VersionInfo } from "./types";
import { apiFetch } from "./loopback-api-client";

/** Dashboard HTTP calls hit the embedded Rust Axum server (`/api/*`), not Python. */

async function jsonFetch<T>(url: string, init?: RequestInit): Promise<T> {
  let res: Response;
  try {
    res = await apiFetch(url, init);
  } catch (err) {
    const reason = err instanceof Error ? err.message : String(err);
    if (/failed to fetch|networkerror|load failed/i.test(reason)) {
      throw new Error(
        `${url} -> backend unavailable (${reason}). Is VoiceSub running at http://127.0.0.1:8765 ?`,
      );
    }
    throw err instanceof Error ? err : new Error(reason);
  }
  if (!res.ok) {
    let detail = "";
    try {
      const body = (await res.json()) as { message?: string };
      if (body?.message) detail = `: ${body.message}`;
    } catch {
      // ignore non-json error bodies
    }
    throw new Error(`${url} -> ${res.status}${detail}`);
  }
  return res.json() as Promise<T>;
}

export async function loadSettings(): Promise<{
  ok: boolean;
  payload: ConfigPayload;
  loaded_from?: string;
  subtitle_style_presets?: import("./types").StylePresetCatalog;
  font_catalog?: import("./types").FontCatalog;
}> {
  return jsonFetch("/api/settings/load");
}

export async function saveSettings(payload: ConfigPayload): Promise<{
  ok: boolean;
  message?: string;
  payload?: ConfigPayload;
  subtitle_style_presets?: import("./types").StylePresetCatalog;
  font_catalog?: import("./types").FontCatalog;
  live_applied?: boolean;
}> {
  return jsonFetch("/api/settings/save", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ payload }),
  });
}

export async function startRuntime(configPayload?: ConfigPayload): Promise<{ ok: boolean; runtime: RuntimeStatus }> {
  return jsonFetch("/api/runtime/start", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ config_payload: configPayload ?? null }),
  });
}

export async function stopRuntime(): Promise<{ ok: boolean; runtime: RuntimeStatus }> {
  return jsonFetch("/api/runtime/stop", { method: "POST" });
}

export async function fetchRuntimeStatus(): Promise<RuntimeStatus> {
  return jsonFetch("/api/runtime/status");
}

export async function fetchObsUrl(): Promise<{ overlay_url: string }> {
  return jsonFetch("/api/obs/url");
}

export async function fetchVersion(): Promise<VersionInfo> {
  return jsonFetch("/api/version");
}

export async function checkUpdates(): Promise<VersionInfo> {
  return jsonFetch("/api/updates/check", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: "{}",
  });
}

export async function listProfiles(): Promise<{ profiles: string[] }> {
  return jsonFetch("/api/profiles");
}

export async function loadProfile(name: string): Promise<{ name: string; payload: ConfigPayload }> {
  return jsonFetch(`/api/profiles/${encodeURIComponent(name)}`);
}

export async function saveProfile(
  name: string,
  payload: ConfigPayload,
): Promise<{ name: string; saved_to?: string; payload: ConfigPayload }> {
  return jsonFetch(`/api/profiles/${encodeURIComponent(name)}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ payload }),
  });
}

async function throwHttpError(res: Response, label: string): Promise<never> {
  let detail = "";
  try {
    const body = (await res.json()) as { message?: string };
    if (body?.message) detail = `: ${body.message}`;
  } catch {
    // ignore non-json error bodies
  }
  throw new Error(`${label} -> ${res.status}${detail}`);
}

export async function deleteProfile(name: string): Promise<{ name: string; deleted: boolean }> {
  const res = await apiFetch(`/api/profiles/${encodeURIComponent(name)}`, { method: "DELETE" });
  if (!res.ok) {
    await throwHttpError(res, "delete profile");
  }
  return res.json() as Promise<{ name: string; deleted: boolean }>;
}

export async function downloadDiagnostics(): Promise<{ filename: string }> {
  const res = await apiFetch("/api/exports/diagnostics");
  if (!res.ok) {
    await throwHttpError(res, "diagnostics export");
  }
  const blob = await res.blob();
  const disposition = res.headers.get("Content-Disposition") || "";
  const match = disposition.match(/filename="([^"]+)"/);
  const filename = match?.[1] || "voicesub-diagnostics.zip";
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
  return { filename };
}

export async function postClientLog(channel: string, message: string, details?: Record<string, unknown>): Promise<void> {
  await apiFetch("/api/logs/client-event", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ channel, message, details }),
  }).catch(() => {});
}

export async function openTtsModule(): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("tts_open_window");
}

export async function openLocalAsrModule(): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("local_asr_open_window");
}

export async function openExternalUrl(url: string): Promise<void> {
  const trimmed = url.trim();
  if (!trimmed) {
    return;
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("open_external_https_url", { url: trimmed });
    return;
  } catch (err) {
    // WebView window.open often does not open the system browser; prefer surfacing
    // the Tauri/shell error when the IPC path was available but rejected the URL.
    const opened = window.open(trimmed, "_blank", "noopener,noreferrer");
    if (opened) {
      return;
    }
    const reason = err instanceof Error ? err.message : String(err);
    throw new Error(`Failed to open external URL: ${reason}`);
  }
}

export async function openLocalUrl(url: string): Promise<void> {
  const trimmed = url.trim();
  if (!trimmed) {
    return;
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("open_local_http_url", { url: trimmed });
  } catch {
    window.open(trimmed, "_blank", "noopener,noreferrer");
  }
}

export async function listRecommendedOpenAiModels(): Promise<{
  models: string[];
  recommended?: boolean;
  source?: string;
}> {
  return jsonFetch("/api/openai/recommended-models");
}

export type OpenAiCompatibleModelsResponse = {
  ok?: boolean;
  models?: string[];
  recommended_models?: string[];
  source?: string;
  error?: string;
  show_all?: boolean;
  base_url?: string;
};

/** Live OpenAI / OpenAI-compatible `GET {base}/models` via local runtime proxy. */
export async function listOpenAiCompatibleModels(options: {
  apiKey?: string;
  baseUrl?: string;
  showAll?: boolean;
}): Promise<OpenAiCompatibleModelsResponse> {
  const res = await apiFetch("/api/openai/models", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      api_key: options.apiKey || "",
      base_url: options.baseUrl || "",
      show_all: options.showAll === true,
    }),
  });
  const body = (await res.json().catch(() => ({}))) as OpenAiCompatibleModelsResponse;
  if (!res.ok) {
    return {
      ok: false,
      models: [],
      error: body.error || `${res.status} ${res.statusText}`,
    };
  }
  return body;
}
