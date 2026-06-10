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

  it("applies a preset to the active style base", () => {
    const next = applyStylePreset(
      { base: { font_size_px: 18 }, line_slots: {} },
      "cyberpunk_neon",
      catalog,
    );
    expect(next.active_preset).toBe("cyberpunk_neon");
    expect(next.base?.fill_color).toBe("#00fff0");
    expect(next.base?.font_size_px).toBe(18);
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
