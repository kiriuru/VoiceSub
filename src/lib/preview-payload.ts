import { t } from "./i18n";
import type { ConfigPayload, RuntimeStatus, StylePresetCatalog } from "./types";

export function hasRenderableOverlayContent(payload: Record<string, unknown> | null | undefined): boolean {
  if (!payload || typeof payload !== "object") return false;
  const visibleItems = Array.isArray(payload.visible_items)
    ? payload.visible_items.filter((item) => String((item as { text?: string })?.text || "").trim())
    : [];
  if (visibleItems.length > 0) return true;
  return Boolean(String(payload.active_partial_text || "").trim());
}

export function shouldUseLiveOverlayPreview(
  runtime: RuntimeStatus | null | undefined,
  overlayPayload: Record<string, unknown> | null | undefined,
): boolean {
  if (!(runtime?.running || runtime?.is_running)) {
    return false;
  }
  return Boolean(overlayPayload);
}

export function getResolvedSubtitleStyle(
  config: ConfigPayload,
  presets: StylePresetCatalog,
): Record<string, unknown> {
  const renderer =
    typeof window !== "undefined"
      ? (
          window as Window & {
            SubtitleStyleRenderer?: {
              resolveEffectiveStyle: (style: unknown, catalog: unknown) => Record<string, unknown>;
            };
          }
        ).SubtitleStyleRenderer
      : undefined;
  if (!renderer) {
    return (config.subtitle_style || {}) as Record<string, unknown>;
  }
  return renderer.resolveEffectiveStyle(config.subtitle_style || {}, presets || {});
}

export function buildPreviewPayload(input: {
  config: ConfigPayload;
  runtime?: RuntimeStatus | null;
  overlayPayload?: Record<string, unknown> | null;
  subtitleStylePresets?: StylePresetCatalog;
  locale?: string;
}): Record<string, unknown> | null {
  const { config, runtime, overlayPayload, subtitleStylePresets = {}, locale } = input;
  const tr = (key: string) => t(key, undefined, locale as import("./types").LocaleCode | undefined);

  if (shouldUseLiveOverlayPreview(runtime, overlayPayload)) {
    return {
      ...(overlayPayload || {}),
      style: getResolvedSubtitleStyle(config, subtitleStylePresets),
    };
  }

  const visibleItems: Array<Record<string, unknown>> = [];
  const displayOrder = Array.isArray(config.subtitle_output?.display_order)
    ? config.subtitle_output.display_order
    : [];
  const maxTranslations = Math.max(
    0,
    Math.min(5, Number(config.subtitle_output?.max_translation_languages || 0)),
  );
  const lineMap = new Map(
    (Array.isArray(config.translation?.lines) ? config.translation.lines : [])
      .filter((line) => line?.enabled !== false)
      .map((line) => [String(line.slot_id || "").toLowerCase(), line]),
  );
  let translationsUsed = 0;

  for (const code of displayOrder) {
    if (code === "source") {
      if (config.subtitle_output?.show_source !== false) {
        visibleItems.push({
          kind: "source",
          lang: config.source_lang || "auto",
          style_slot: "source",
          text: tr("preview.source_line"),
        });
      }
      continue;
    }
    if (config.subtitle_output?.show_translations === false || translationsUsed >= maxTranslations) {
      continue;
    }
    const line = lineMap.get(String(code || "").toLowerCase());
    if (!line) continue;
    visibleItems.push({
      kind: "translation",
      lang: String(line.target_lang || code),
      slot_id: String(line.slot_id || code),
      target_lang: String(line.target_lang || code),
      label: String(line.label || String(line.target_lang || code).toUpperCase()),
      style_slot: String(line.slot_id || code),
      text: String(line.label || line.target_lang || code),
    });
    translationsUsed += 1;
  }

  return {
    preset: config.overlay?.preset || "single",
    compact: Boolean(config.overlay?.compact),
    completed_block_visible: visibleItems.length > 0,
    visible_items: visibleItems,
    active_partial_text:
      visibleItems.length === 0 && config.subtitle_output?.show_source !== false
        ? tr("preview.live_partial")
        : "",
    style: getResolvedSubtitleStyle(config, subtitleStylePresets),
    sequence: 0,
  };
}
