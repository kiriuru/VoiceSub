import { REDACTED_VALUE } from "./redaction";

/** True when a config JSON still contains redaction placeholders (unsafe to import as live secrets). */
export function containsRedactedPlaceholders(value: unknown): boolean {
  if (value === REDACTED_VALUE) return true;
  if (typeof value === "string") {
    return value.trim() === REDACTED_VALUE;
  }
  if (Array.isArray(value)) {
    return value.some(containsRedactedPlaceholders);
  }
  if (value && typeof value === "object") {
    return Object.values(value as Record<string, unknown>).some(containsRedactedPlaceholders);
  }
  return false;
}
