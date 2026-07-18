/** Defaults for subtitle style base — keep in sync with subtitle-style.js DEFAULT_BASE_STYLE. */
export const STYLE_BASE_DEFAULTS: Record<string, string | number> = {
  font_family: '"Segoe UI", Tahoma, Geneva, Verdana, sans-serif',
  font_size_px: 30,
  font_weight: 700,
  fill_color: "#ffffff",
  stroke_color: "#000000",
  stroke_width_px: 2,
  shadow_color: "#000000",
  shadow_blur_px: 10,
  shadow_offset_x_px: 0,
  shadow_offset_y_px: 3,
  background_color: "#000000",
  background_opacity: 0,
  background_padding_x_px: 12,
  background_padding_y_px: 4,
  background_radius_px: 10,
  line_spacing_em: 1.15,
  letter_spacing_em: 0,
  text_align: "center",
  line_gap_px: 8,
  effect: "none",
};

/** Fields that only affect the stage container (not per-slot surfaces). */
export const CONTAINER_ONLY_STYLE_FIELDS = ["line_gap_px"] as const;

/**
 * ASS/Aegisub-style outline thickness in CSS pixels: 0–4, step 0.1.
 * Classic SSA listed 0–4; VSFilter/`\bord` accepts floats (0.1, 1.5, …).
 */
export const STROKE_WIDTH_MIN = 0;
export const STROKE_WIDTH_MAX = 4;
export const STROKE_WIDTH_STEP = 0.1;

/** Clamp outline width to the caption scale (hides legacy 5–20 values in the UI). */
export function clampStrokeWidthPx(raw: unknown, fallback = 2): number {
  const parsed = Number(raw);
  const value = Number.isFinite(parsed) ? parsed : fallback;
  const clamped = Math.min(STROKE_WIDTH_MAX, Math.max(STROKE_WIDTH_MIN, value));
  return Number(clamped.toFixed(1));
}

/** Normalize a color for `<input type="color">` (#rrggbb). */
export function toCssColorInput(raw: string, fallback = "#000000"): string {
  const value = String(raw || "").trim();
  if (/^#[0-9a-fA-F]{6}$/.test(value)) {
    return value.toLowerCase();
  }
  if (/^#[0-9a-fA-F]{3}$/.test(value)) {
    const r = value[1];
    const g = value[2];
    const b = value[3];
    return `#${r}${r}${g}${g}${b}${b}`.toLowerCase();
  }
  if (/^#[0-9a-fA-F]{8}$/.test(value)) {
    return `#${value.slice(1, 7)}`.toLowerCase();
  }
  const fb = String(fallback || "#000000").trim();
  return /^#[0-9a-fA-F]{6}$/.test(fb) ? fb.toLowerCase() : "#000000";
}
