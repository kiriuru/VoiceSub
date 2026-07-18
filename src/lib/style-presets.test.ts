import { describe, expect, it } from "vitest";

import {
  applyPresetToLineSlot,
  applyStylePreset,
  deleteCustomPreset,
  normalizePresetKey,
  saveCustomPreset,
} from "./style-presets";

const catalog = {
  clean_default: {
    label: "Clean Default",
    built_in: true,
    base: {
      font_family: '"Inter Regular", "Segoe UI", sans-serif',
      font_size_px: 30,
      effect: "none",
    },
    line_slots: {},
  },
  cyberpunk_neon: {
    label: "Cyberpunk Neon",
    built_in: true,
    base: {
      font_family: '"Orbitron Black", "Exo 2 Black", sans-serif',
      fill_color: "#00fff0",
      effect: "glow",
    },
    line_slots: {},
  },
} as const;

describe("style preset helpers", () => {
  it("normalizes custom preset keys", () => {
    expect(normalizePresetKey(" My Stream Style ")).toBe("my_stream_style");
  });

  it("applies a preset to the active style base without stale prior metrics", () => {
    const next = applyStylePreset(
      {
        base: {
          font_size_px: 18,
          letter_spacing_em: 0.4,
          stroke_width_px: 7,
          background_opacity: 55,
        },
        line_slots: {},
      },
      "cyberpunk_neon",
      catalog,
    );
    expect(next.active_preset).toBe("cyberpunk_neon");
    expect(next.base?.fill_color).toBe("#00fff0");
    expect(next.base?.effect).toBe("glow");
    // Incomplete catalog entry → defaults, not leftover values from the prior base.
    expect(next.base?.font_size_px).toBe(30);
    expect(next.base?.letter_spacing_em).toBe(0);
    expect(next.base?.stroke_width_px).toBe(2);
    expect(next.base?.background_opacity).toBe(0);
  });

  it("copies every numeric metric from a full preset base", () => {
    const fullCatalog = {
      streamer_bold: {
        label: "Streamer Neon",
        built_in: true,
        base: {
          font_family: '"Oswald Bold", sans-serif',
          font_size_px: 36,
          font_weight: 800,
          fill_color: "#00e8ff",
          stroke_color: "#07040f",
          stroke_width_px: 3,
          shadow_color: "#ff2bd6",
          shadow_blur_px: 16,
          shadow_offset_x_px: 0,
          shadow_offset_y_px: 0,
          background_color: "#000000",
          background_opacity: 0,
          background_padding_x_px: 12,
          background_padding_y_px: 4,
          background_radius_px: 8,
          line_spacing_em: 1.15,
          letter_spacing_em: 0.015,
          text_align: "center",
          line_gap_px: 8,
          effect: "glow",
        },
        line_slots: {},
      },
    };
    const next = applyStylePreset({ base: {}, line_slots: {} }, "streamer_bold", fullCatalog);
    expect(next.base).toMatchObject(fullCatalog.streamer_bold.base);
  });

  it("applies a preset base to a line slot and forces enabled", () => {
    const style = {
      base: { font_size_px: 30, effect: "none" },
      line_slots: {
        source: { enabled: false },
      },
    };
    const next = applyPresetToLineSlot(style, "source", "cyberpunk_neon", catalog);
    expect(next.line_slots?.source?.enabled).toBe(true);
    expect(next.line_slots?.source?.fill_color).toBe("#00fff0");
    expect(next.line_slots?.source?.effect).toBe("glow");
  });

  it("keeps numeric stroke_width_px when applying a preset to a line slot", () => {
    const numericCatalog = {
      ...catalog,
      comic_burst: {
        label: "Comic Burst",
        built_in: true,
        base: {
          stroke_width_px: 4,
          font_size_px: 42,
          effect: "zoom_in",
        },
        line_slots: {},
      },
      compact_overlay: {
        label: "Compact Bar",
        built_in: true,
        base: {
          stroke_width_px: 0,
          font_size_px: 24,
          effect: "none",
        },
        line_slots: {},
      },
    };
    const style = {
      base: { stroke_width_px: 2 },
      line_slots: { source: { enabled: false } },
    };

    const thick = applyPresetToLineSlot(style, "source", "comic_burst", numericCatalog);
    expect(thick.line_slots?.source?.stroke_width_px).toBe(4);
    expect(typeof thick.line_slots?.source?.stroke_width_px).toBe("number");

    const none = applyPresetToLineSlot(style, "source", "compact_overlay", numericCatalog);
    expect(none.line_slots?.source?.stroke_width_px).toBe(0);
    expect(typeof none.line_slots?.source?.stroke_width_px).toBe("number");
  });

  it("roundtrips custom presets through save and delete", () => {
    const saved = saveCustomPreset(
      { base: { font_size_px: 44 }, line_slots: {} },
      "My Stream",
      catalog,
    );
    expect(saved.active_preset).toBe("my_stream");
    expect(saved.custom_presets).toBeTruthy();

    const deleted = deleteCustomPreset(saved, "My Stream");
    expect(deleted.custom_presets).not.toHaveProperty("my_stream");
    expect(deleted.active_preset).toBe("clean_default");
  });
});
