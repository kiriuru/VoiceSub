import { describe, expect, it } from "vitest";

import {
  clampStrokeWidthPx,
  STYLE_BASE_DEFAULTS,
  STROKE_WIDTH_MAX,
  toCssColorInput,
} from "./style-field-utils";

describe("style field utils", () => {
  it("clamps outline width to ASS-style 0–4 px with 0.1 precision", () => {
    expect(STROKE_WIDTH_MAX).toBe(4);
    expect(clampStrokeWidthPx(0.1)).toBe(0.1);
    expect(clampStrokeWidthPx(1.5)).toBe(1.5);
    expect(clampStrokeWidthPx(4)).toBe(4);
    expect(clampStrokeWidthPx(4.5)).toBe(4);
    expect(clampStrokeWidthPx(10)).toBe(4);
    expect(clampStrokeWidthPx(-1)).toBe(0);
    expect(clampStrokeWidthPx("1.25")).toBe(1.3);
  });

  it("normalizes colors for HTML color inputs", () => {
    expect(toCssColorInput("#ABC")).toBe("#aabbcc");
    expect(toCssColorInput("#ff00ffaa")).toBe("#ff00ff");
    expect(toCssColorInput("#00E8FF")).toBe("#00e8ff");
    expect(toCssColorInput("not-a-color", "#ffffff")).toBe("#ffffff");
  });

  it("exposes a complete default base used when applying presets", () => {
    const required = [
      "font_family",
      "font_size_px",
      "font_weight",
      "fill_color",
      "stroke_color",
      "stroke_width_px",
      "shadow_color",
      "shadow_blur_px",
      "shadow_offset_x_px",
      "shadow_offset_y_px",
      "background_color",
      "background_opacity",
      "background_padding_x_px",
      "background_padding_y_px",
      "background_radius_px",
      "line_spacing_em",
      "letter_spacing_em",
      "text_align",
      "line_gap_px",
      "effect",
    ];
    for (const key of required) {
      expect(STYLE_BASE_DEFAULTS[key], key).toBeDefined();
    }
  });
});
