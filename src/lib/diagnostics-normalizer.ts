export function normalizeDiagnosticsPayload(
  payload: Record<string, unknown> | null | undefined,
  previous?: Record<string, unknown> | null,
) {
  const current = payload && typeof payload === "object" ? payload : {};
  const prev = previous && typeof previous === "object" ? previous : {};
  const localModule =
    current.local_module && typeof current.local_module === "object"
      ? { ...(current.local_module as Record<string, unknown>) }
      : prev.local_module && typeof prev.local_module === "object"
        ? { ...(prev.local_module as Record<string, unknown>) }
        : undefined;
  const activeMode = current.active_mode || prev.active_mode;
  return {
    provider: String(current.provider || activeMode || ""),
    active_mode: activeMode ? String(activeMode) : undefined,
    selected_device: String(current.selected_device || ""),
    selected_execution_provider: String(current.selected_execution_provider || ""),
    partials_supported: current.partials_supported === true,
    browser_worker:
      current.browser_worker && typeof current.browser_worker === "object"
        ? { ...(current.browser_worker as Record<string, unknown>) }
        : null,
    local_module: localModule,
    message: String(current.message || ""),
    degraded_mode: current.degraded_mode === true,
    partial_emit_mode: String(current.partial_emit_mode || ""),
    partial_min_new_words:
      typeof current.partial_min_new_words === "number" && Number.isFinite(current.partial_min_new_words)
        ? current.partial_min_new_words
        : null,
    true_streaming: current.true_streaming === true,
    raw: current,
  };
}
