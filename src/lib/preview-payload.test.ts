import { describe, expect, it } from "vitest";

import {
  buildPreviewPayload,
  hasRenderableOverlayContent,
  shouldUseLiveOverlayPreview,
} from "./preview-payload";
import type { ConfigPayload } from "./types";

const config: ConfigPayload = {
  overlay: { preset: "single", compact: false },
  subtitle_output: {
    show_source: true,
    show_translations: true,
    max_translation_languages: 1,
    display_order: ["source", "translation_1"],
  },
  source_lang: "en",
  translation: {
    enabled: true,
    lines: [
      {
        slot_id: "translation_1",
        target_lang: "en",
        label: "EN",
        enabled: true,
        provider: "google_translate_v2",
      },
    ],
  },
  subtitle_style: {},
};

const idleEmptyPayload = {
  visible_items: [],
  completed_block_visible: true,
  active_partial_text: "",
};

describe("preview payload idle/live gating", () => {
  it("treats empty replay payload as non-renderable", () => {
    expect(hasRenderableOverlayContent(idleEmptyPayload)).toBe(false);
  });

  it("does not use live overlay preview while runtime is idle", () => {
    expect(
      shouldUseLiveOverlayPreview({ is_running: false }, idleEmptyPayload),
    ).toBe(false);
    expect(
      shouldUseLiveOverlayPreview(
        { is_running: false },
        {
          lifecycle_state: "completed_only",
          visible_items: [{ kind: "translation", text: "stale subtitle" }],
          active_partial_text: "",
        },
      ),
    ).toBe(false);
  });

  it("builds style placeholder before start", () => {
    const placeholder = buildPreviewPayload({
      config,
      runtime: { is_running: false },
      overlayPayload: idleEmptyPayload,
      subtitleStylePresets: {},
      locale: "en",
    });
    expect(placeholder?.visible_items?.length).toBeGreaterThan(0);
    expect(placeholder?.visible_items?.[0]?.kind).toBe("source");
  });

  it("uses live overlay payload once runtime is running", () => {
    const livePayload = {
      ...idleEmptyPayload,
      sequence: 7,
      lifecycle_state: "partial_only",
      active_partial_text: "live",
    };
    expect(
      shouldUseLiveOverlayPreview({ is_running: true, running: true }, livePayload),
    ).toBe(true);

    const preview = buildPreviewPayload({
      config,
      runtime: { is_running: true, running: true },
      overlayPayload: livePayload,
      subtitleStylePresets: {},
      locale: "en",
    });
    expect(preview?.active_partial_text).toBe("live");
    expect(preview?.sequence).toBe(7);
  });
});
