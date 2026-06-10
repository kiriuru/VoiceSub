const LIFECYCLE_STATES = new Set([
  "idle",
  "partial_only",
  "completed_only",
  "completed_with_partial",
]);

export function normalizeOverlayPayload(
  payload: Record<string, unknown> | null | undefined,
): Record<string, unknown> {
  const current = payload && typeof payload === "object" ? payload : {};
  const rawLifecycle = String(current.lifecycle_state || "idle");
  const lifecycle_state = LIFECYCLE_STATES.has(rawLifecycle) ? rawLifecycle : "idle";
  return {
    sequence: Number.isFinite(Number(current.sequence)) ? Number(current.sequence) : 0,
    event_sequence: Number.isFinite(Number(current.event_sequence)) ? Number(current.event_sequence) : 0,
    created_at_ms: Number.isFinite(Number(current.created_at_ms)) ? Number(current.created_at_ms) : 0,
    preset: ["single", "dual-line", "stacked"].includes(String(current.preset || "stacked"))
      ? String(current.preset || "stacked")
      : "stacked",
    compact: current.compact === true,
    completed_block_visible: current.completed_block_visible === true,
    lifecycle_state,
    show_source: current.show_source !== false,
    show_translations: current.show_translations !== false,
    active_partial_text: String(current.active_partial_text || ""),
    visible_items: (Array.isArray(current.visible_items) ? current.visible_items : []).map((item) => ({
      kind: String((item as { kind?: string })?.kind || "source"),
      lang: String((item as { lang?: string })?.lang || ""),
      slot_id: String((item as { slot_id?: string })?.slot_id || ""),
      target_lang: String((item as { target_lang?: string })?.target_lang || ""),
      label: String((item as { label?: string })?.label || ""),
      provider: String((item as { provider?: string })?.provider || ""),
      text: String((item as { text?: string })?.text || ""),
      style_slot: String((item as { style_slot?: string })?.style_slot || ""),
    })),
    style: current.style && typeof current.style === "object" ? { ...(current.style as object) } : {},
  };
}
