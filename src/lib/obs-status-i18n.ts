type TranslateFn = (key: string, vars?: Record<string, string>) => string;

const OBS_ERROR_I18N_KEYS: Record<string, string> = {
  password_required: "obs.cc.error.password_required",
  auth_failed: "obs.cc.error.auth_failed",
  connection_refused: "obs.cc.error.connection_refused",
  connection_timeout: "obs.cc.error.connection_timeout",
  connection_failed: "obs.cc.error.connection_failed",
  connection_lost: "obs.cc.error.connection_lost",
  protocol_error: "obs.cc.error.protocol_error",
  request_failed: "obs.cc.error.request_failed",
  not_connected: "obs.cc.error.not_connected",
  send_failed: "obs.cc.error.send_failed",
  stream_not_running: "obs.cc.error.stream_not_running",
  captions_unavailable: "obs.cc.error.connection_failed",
};

const OBS_NATIVE_STATUS_I18N_KEYS: Record<string, string> = {
  not_connected: "obs.cc.native.not_connected",
  stream_active: "obs.cc.native.stream_active",
  stream_active_reconnecting: "obs.cc.native.stream_active_reconnecting",
  stream_inactive: "obs.cc.native.stream_inactive",
  stream_not_running: "obs.cc.native.stream_not_running",
  stream_delivered: "obs.cc.native.stream_delivered",
  readiness_pending: "obs.cc.native.readiness_pending",
};

export function resolveObsErrorCode(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) return "";
  if (/^[a-z][a-z0-9_]*$/.test(trimmed)) {
    return trimmed;
  }

  const lower = trimmed.toLowerCase();
  if (lower.includes("password") && (lower.includes("configured") || lower.includes("none"))) {
    return "password_required";
  }
  if (lower.includes("authentication failed") || lower.includes("auth failed")) {
    return "auth_failed";
  }
  if (
    lower.includes("10061") ||
    lower.includes("connection refused") ||
    lower.includes("actively refused") ||
    lower.includes("отверг запрос на подключение")
  ) {
    return "connection_refused";
  }
  if (lower.includes("10060") || lower.includes("timed out") || lower.includes("timeout")) {
    return "connection_timeout";
  }
  if (lower.includes("connection lost")) {
    return "connection_lost";
  }
  if (lower.includes("not connected")) {
    return "not_connected";
  }
  if (lower.includes("stream output is not running") || lower.includes("not active")) {
    return "stream_not_running";
  }
  if (lower.includes("send failed")) {
    return "send_failed";
  }
  if (lower.includes("protocol error") || lower.includes("send timeout") || lower.includes("recv timeout")) {
    return "protocol_error";
  }
  if (lower.includes("request failed")) {
    return "request_failed";
  }
  if (lower.includes("captions unavailable") || lower.includes("io error")) {
    return "connection_failed";
  }
  return "generic";
}

export function formatObsCaptionError(raw: string | undefined | null, tr: TranslateFn): string {
  const code = resolveObsErrorCode(String(raw || ""));
  const i18nKey = OBS_ERROR_I18N_KEYS[code] || "obs.cc.error.generic";
  const translated = tr(i18nKey);
  if (translated !== i18nKey) {
    return translated;
  }
  return tr("obs.cc.error.generic");
}

const OBS_CONNECTION_STATE_I18N_KEYS: Record<string, string> = {
  disabled: "obs.cc.connection_state.disabled",
  disconnected: "obs.cc.connection_state.disconnected",
  connecting: "obs.cc.connection_state.connecting",
  connected: "obs.cc.connection_state.connected",
  auth_failed: "obs.cc.connection_state.auth_failed",
  error: "obs.cc.connection_state.error",
};

export function formatObsConnectionState(
  raw: string | undefined | null,
  tr: TranslateFn,
): string {
  const key = String(raw || "disabled").trim().toLowerCase() || "disabled";
  const i18nKey = OBS_CONNECTION_STATE_I18N_KEYS[key] || OBS_CONNECTION_STATE_I18N_KEYS.error;
  const translated = tr(i18nKey);
  return translated !== i18nKey ? translated : key;
}

export function formatObsCcRuntimeStatus(
  diagnostics: Record<string, unknown> | undefined,
  tr: TranslateFn,
): string {
  const diag = diagnostics || {};
  const state = formatObsConnectionState(String(diag.connection_state || "disabled"), tr);
  const lastError = String(diag.last_error || "").trim();
  const errorSuffix = lastError ? ` · ${formatObsCaptionError(lastError, tr)}` : "";
  return tr("tools.runtime.obs_cc_status", { state, error: errorSuffix });
}

export function formatObsNativeCaptionStatus(
  raw: string | undefined | null,
  tr: TranslateFn,
): string {
  const trimmed = String(raw || "").trim();
  if (!trimmed) return "";
  const code = /^[a-z][a-z0-9_]*$/.test(trimmed) ? trimmed : resolveObsErrorCode(trimmed);
  const key = OBS_NATIVE_STATUS_I18N_KEYS[code];
  return key ? tr(key) : trimmed;
}

export function formatObsOutputMode(mode: string | undefined | null, tr: TranslateFn): string {
  const key = String(mode || "disabled").trim() || "disabled";
  const i18nKey = `obs.output.${key}`;
  const translated = tr(i18nKey);
  return translated === i18nKey ? key : translated;
}
