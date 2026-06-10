/** Port of `frontend/js/core/redaction.js` and `voicesub-logging/redaction.rs`. */

export const REDACTED_VALUE = "[redacted]";

const SENSITIVE_KEYS = new Set([
  "api_key",
  "key",
  "q",
  "text",
  "token",
  "secret",
  "password",
  "authorization",
  "credential",
  "credentials",
  "pair_code",
  "local_admin_token",
  "bearer",
]);

const SENSITIVE_FRAGMENTS = [
  "api_key",
  "token",
  "secret",
  "password",
  "authorization",
  "credential",
  "pair_code",
  "local_admin_token",
  "bearer",
];

const BEARER_PATTERN = /\bbearer\s+([^\s,;]+)/gi;
const QUERY_PARAM_PATTERN =
  /\b(api_key|key|token|secret|password|authorization|credential|credentials|pair_code|local_admin_token|bearer)=([^&\s]+)/gi;

export function isSensitiveKey(key: string | undefined | null): boolean {
  const normalized = String(key || "").trim().toLowerCase();
  if (!normalized) return false;
  if (SENSITIVE_KEYS.has(normalized)) return true;
  return SENSITIVE_FRAGMENTS.some((fragment) => normalized.includes(fragment));
}

export function redactText(value: unknown): string {
  const text = String(value ?? "");
  if (!text) return text;
  return text
    .replace(BEARER_PATTERN, "Bearer [redacted]")
    .replace(QUERY_PARAM_PATTERN, (_match, key: string) => `${key}=${REDACTED_VALUE}`);
}

export function redactUrl(value: unknown): string {
  const raw = String(value ?? "").trim();
  if (!raw) return raw;
  try {
    const url = new URL(raw, "http://127.0.0.1/");
    let changed = false;
    [...url.searchParams.keys()].forEach((key) => {
      if (!isSensitiveKey(key)) return;
      url.searchParams.set(key, REDACTED_VALUE);
      changed = true;
    });
    return changed ? url.toString() : raw;
  } catch {
    return redactText(raw);
  }
}

export function redactValue(value: unknown, key?: string): unknown {
  if (isSensitiveKey(key)) return REDACTED_VALUE;
  if (Array.isArray(value)) return value.map((item) => redactValue(item));
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([childKey, childValue]) => [
        childKey,
        redactValue(childValue, childKey),
      ]),
    );
  }
  if (typeof value === "string") {
    const normalizedKey = String(key || "").trim().toLowerCase();
    if (normalizedKey === "endpoint") {
      const redactedUrl = redactUrl(value);
      if (redactedUrl !== value) return redactedUrl;
      if (value.toLowerCase().includes("secret")) return REDACTED_VALUE;
    }
    return redactText(value);
  }
  return value;
}

export function redactObject<T>(value: T): T {
  return redactValue(value) as T;
}
