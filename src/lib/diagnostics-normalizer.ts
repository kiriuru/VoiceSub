export function normalizeDiagnosticsPayload(payload: Record<string, unknown> | null | undefined) {
  const current = payload && typeof payload === "object" ? payload : {};
  return {
    provider: String(current.provider || ""),
    selected_device: String(current.selected_device || ""),
    selected_execution_provider: String(current.selected_execution_provider || ""),
    partials_supported: current.partials_supported === true,
    browser_worker:
      current.browser_worker && typeof current.browser_worker === "object"
        ? { ...(current.browser_worker as Record<string, unknown>) }
        : null,
    message: String(current.message || ""),
    fallback_reason: String(current.fallback_reason || ""),
    cpu_fallback_reason: String(current.cpu_fallback_reason || ""),
    requested_device_policy: String(current.requested_device_policy || ""),
    torch_built_with_cuda: current.torch_built_with_cuda === true,
    degraded_mode: current.degraded_mode === true,
    active_latency_preset: String(current.active_latency_preset || ""),
    streaming_decode:
      current.streaming_decode === false ? false : current.streaming_decode === true ? true : null,
    partial_emit_mode: String(current.partial_emit_mode || ""),
    partial_min_new_words:
      typeof current.partial_min_new_words === "number" && Number.isFinite(current.partial_min_new_words)
        ? current.partial_min_new_words
        : null,
    true_streaming: current.true_streaming === true,
    raw: current,
  };
}
