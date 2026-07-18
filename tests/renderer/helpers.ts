export type RenderTraceEvent = Record<string, unknown>;

export type SubtitleStyleRenderer = {
  render: (
    container: HTMLElement,
    payload: Record<string, unknown>,
    options?: Record<string, unknown>,
  ) => { empty: boolean; rowCount?: number };
  composeRenderRows: (payload: Record<string, unknown>) => Array<{
    rowSlot: string;
    entries: Array<{
      kind: string;
      text: string;
      style_slot: string;
      transient?: boolean;
    }>;
  }>;
  classifyPartialTransition: (
    currentText: string,
    previousText: string,
    sharedLength: number,
  ) => string;
  commonPrefixLength: (a: string, b: string) => number;
  colorToRgba: (color: string, opacityPercent: number) => string;
  resolveFreshFragmentEffect: (
    slotEffect: string,
    options: Record<string, unknown> | undefined,
    deltaLength: number,
    totalLength: number,
  ) => string;
  disposeRenderContainer: (container: HTMLElement) => void;
  normalizeBaseStyle?: (raw: Record<string, unknown>) => Record<string, unknown>;
  buildCssVariables?: (
    roleStyle: Record<string, unknown>,
    scale?: number,
  ) => Record<string, string>;
  buildOutlineTextShadow?: (widthPx: number, color: string) => string;
  resolveEffectiveStyle?: (
    rawStyle: Record<string, unknown>,
    presets?: Record<string, unknown>,
  ) => Record<string, unknown>;
  OVERLAY_DENSE_PARTIAL_CHARS?: number;
  OVERLAY_MAX_ANIMATED_DELTA_CHARS?: number;
};

declare global {
  interface Window {
    SubtitleStyleRenderer: SubtitleStyleRenderer;
  }

  interface HTMLElement {
    __subtitleStyleRenderState?: unknown;
  }
}

export function renderer(): SubtitleStyleRenderer {
  if (!window.SubtitleStyleRenderer) {
    throw new Error("SubtitleStyleRenderer is not loaded");
  }
  return window.SubtitleStyleRenderer;
}

export function minimalStyle(): Record<string, unknown> {
  return {
    preset: "clean_default",
    base: {
      font_family: "Inter, sans-serif",
      font_size_px: 30,
      fill_color: "#ffffff",
      stroke_color: "#000000",
      stroke_width_px: 2,
      effect: "none",
      background_opacity: 0,
    },
    line_slots: {},
  };
}

export function partialOnlyPayload(
  text: string,
  extra: Record<string, unknown> = {},
): Record<string, unknown> {
  return {
    preset: "single",
    compact: false,
    lifecycle_state: "partial_only",
    completed_block_visible: false,
    active_partial_text: text,
    show_source: true,
    show_translations: false,
    visible_items: [],
    style: minimalStyle(),
    ...extra,
  };
}

export function collectTrace(
  container: HTMLElement,
  payload: Record<string, unknown>,
  options: Record<string, unknown> = {},
): RenderTraceEvent[] {
  const events: RenderTraceEvent[] = [];
  renderer().render(container, payload, {
    overlay: true,
    ...options,
    onRenderTrace: (event: RenderTraceEvent) => {
      events.push(event);
    },
  });
  return events;
}
