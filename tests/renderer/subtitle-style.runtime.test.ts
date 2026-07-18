import { beforeEach, describe, expect, it } from "vitest";

import {
  collectTrace,
  minimalStyle,
  partialOnlyPayload,
  renderer,
} from "./helpers";

describe("SubtitleStyleRenderer runtime", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  it("classifies partial transitions", () => {
    const R = renderer();
    expect(R.classifyPartialTransition("hello", "", 0)).toBe("initial");
    expect(R.classifyPartialTransition("hello", "hello", 5)).toBe("identical");
    expect(R.classifyPartialTransition("hello!", "hello", 5)).toBe("extension");
    expect(R.classifyPartialTransition("hell", "hello", 4)).toBe("shrink");
    expect(R.classifyPartialTransition("world", "hello", 0)).toBe("jump");
    expect(R.classifyPartialTransition("helo", "hello", 2)).toBe("revision");
  });

  it("computes shared prefix length", () => {
    const R = renderer();
    expect(R.commonPrefixLength("hello", "help")).toBe(3);
    expect(R.commonPrefixLength("", "abc")).toBe(0);
  });

  it("does not split emoji surrogate pairs in commonPrefixLength", () => {
    const R = renderer();
    const waving = "Hi👋";
    const waveOnly = "👋";
    expect(R.commonPrefixLength(`${waving} there`, waving)).toBe(waving.length);
    // Differing emoji after a shared ASCII prefix must not leave a lone high surrogate.
    expect(R.commonPrefixLength(`A${waveOnly}`, "A😀")).toBe(1);
  });

  it("preserves non-contiguous style_slot overrides in composeRenderRows", () => {
    const rows = renderer().composeRenderRows({
      preset: "stacked",
      lifecycle_state: "completed_only",
      completed_block_visible: true,
      active_partial_text: "",
      show_source: false,
      show_translations: true,
      visible_items: [
        {
          kind: "translation",
          text: "こんにちは",
          style_slot: "translation_2",
          slot_id: "translation_2",
          lang: "ja",
        },
      ],
    });
    expect(rows).toHaveLength(1);
    expect(rows[0]?.entries[0]?.style_slot).toBe("translation_2");
  });

  it("falls back to slot_id when style_slot is missing", () => {
    const rows = renderer().composeRenderRows({
      preset: "stacked",
      lifecycle_state: "completed_only",
      completed_block_visible: true,
      active_partial_text: "",
      visible_items: [
        {
          kind: "translation",
          text: "hola",
          slot_id: "translation_3",
          lang: "es",
        },
      ],
    });
    expect(rows[0]?.entries[0]?.style_slot).toBe("translation_3");
  });

  it("filters whitespace-only visible items", () => {
    const rows = renderer().composeRenderRows({
      preset: "stacked",
      lifecycle_state: "completed_only",
      completed_block_visible: true,
      active_partial_text: "",
      visible_items: [
        { kind: "source", text: "   ", style_slot: "source" },
        { kind: "translation", text: "ok", style_slot: "translation_1" },
      ],
    });
    expect(rows).toHaveLength(1);
    expect(rows[0]?.entries[0]?.text).toBe("ok");
  });

  it("colorToRgba supports hex8, rgb(), and named colors with opacity", () => {
    const R = renderer();
    expect(R.colorToRgba("#112233", 50)).toBe("rgba(17, 34, 51, 0.50)");
    expect(R.colorToRgba("112233", 50)).toBe("rgba(17, 34, 51, 0.50)");
    expect(R.colorToRgba("#11223380", 100)).toBe("rgba(17, 34, 51, 0.50)");
    expect(R.colorToRgba("rgb(10, 20, 30)", 50)).toBe("rgba(10, 20, 30, 0.50)");
    expect(R.colorToRgba("black", 50)).toBe("rgba(0, 0, 0, 0.50)");
    expect(R.colorToRgba("red", 0)).toBe("transparent");
  });

  it("empty render returns empty without building a stage shell", () => {
    const R = renderer();
    R.render(container, partialOnlyPayload("Hi"), { overlay: true });
    expect(container.firstElementChild).toBeTruthy();

    const result = R.render(
      container,
      {
        preset: "single",
        lifecycle_state: "idle",
        completed_block_visible: false,
        active_partial_text: "",
        visible_items: [],
        style: minimalStyle(),
      },
      { overlay: true },
    );
    expect(result.empty).toBe(true);
    // Early empty path leaves prior DOM for the caller to dispose.
    expect(container.firstElementChild).toBeTruthy();
    R.disposeRenderContainer(container);
    expect(container.childNodes.length).toBe(0);
  });

  it("falls back to slow path when a prior surface is disconnected", () => {
    const R = renderer();
    R.render(container, partialOnlyPayload("Hel"), { overlay: true });
    const surface = container.querySelector(".subtitle-line__surface") as HTMLElement | null;
    expect(surface).toBeTruthy();
    surface?.remove();

    const traces = collectTrace(container, partialOnlyPayload("Hello"));
    const summary = traces.find((event) => event.type === "render_summary");
    expect(summary?.fast_path).toBe(false);
    expect(container.textContent).toContain("Hello");
    expect(container.textContent).not.toContain("Hellolo");
  });

  it("creates an empty static span on the first partial frame", () => {
    renderer().render(container, partialOnlyPayload("H"), { overlay: true });
    const staticSpan = container.querySelector(".subtitle-fragment-static");
    const freshSpan = container.querySelector(".subtitle-fragment-fresh");
    expect(staticSpan).toBeTruthy();
    expect(staticSpan?.textContent).toBe("");
    expect(freshSpan?.textContent).toBe("H");
  });

  it("reuses the wrapper on a pure partial extension", () => {
    const R = renderer();
    R.render(container, partialOnlyPayload("Hel"), { overlay: true });
    const wrapper = container.firstElementChild;
    expect(wrapper).toBeTruthy();

    const traces = collectTrace(container, partialOnlyPayload("Hello"));
    expect(container.firstElementChild).toBe(wrapper);

    const summary = traces.find((event) => event.type === "render_summary");
    expect(summary?.fast_path).toBe(true);

    const staticSpan = container.querySelector(".subtitle-fragment-static");
    const freshSpan = container.querySelector(".subtitle-fragment-fresh");
    expect(staticSpan?.textContent).toBe("Hel");
    expect(freshSpan?.textContent).toBe("lo");
  });

  it("finalizes a partial source in place without rebuilding the wrapper", () => {
    const R = renderer();
    const style = minimalStyle();
    const base = {
      preset: "single",
      compact: false,
      show_source: true,
      show_translations: false,
      style,
    };

    R.render(
      container,
      {
        ...base,
        lifecycle_state: "partial_only",
        completed_block_visible: false,
        active_partial_text: "Hello",
        visible_items: [],
      },
      { overlay: true },
    );
    const wrapper = container.firstElementChild;
    expect(wrapper).toBeTruthy();

    const traces = collectTrace(container, {
      ...base,
      lifecycle_state: "completed_only",
      completed_block_visible: true,
      active_partial_text: "",
      visible_items: [{ kind: "source", text: "Hello", style_slot: "source" }],
    });

    expect(container.firstElementChild).toBe(wrapper);
    const summary = traces.find((event) => event.type === "render_summary");
    expect(summary?.fast_path).toBe(true);
    expect(Number(summary?.finalized_in_place || 0)).toBeGreaterThan(0);
    expect(container.querySelector(".subtitle-fragment-fresh")).toBeNull();
  });

  it("marks live partial source as transient for completed_with_partial", () => {
    const rows = renderer().composeRenderRows({
      preset: "stacked",
      lifecycle_state: "completed_with_partial",
      completed_block_visible: true,
      active_partial_text: "New phrase",
      show_source: true,
      show_translations: true,
      visible_items: [
        { kind: "source", text: "New phrase", style_slot: "source" },
        {
          kind: "translation",
          text: "Old translation",
          style_slot: "translation_1",
          lang: "en",
        },
      ],
    });

    const entries = rows.flatMap((row) => row.entries);
    const source = entries.find((entry) => entry.kind === "source");
    const translation = entries.find((entry) => entry.kind === "translation");
    expect(source?.transient).toBe(true);
    expect(translation?.transient).toBeFalsy();
  });

  it("flags recogniser revision without rebuilding the wrapper", () => {
    const R = renderer();
    R.render(container, partialOnlyPayload("hello"), { overlay: true });
    const wrapper = container.firstElementChild;

    const traces = collectTrace(container, partialOnlyPayload("helo"));
    const partial = traces.find((event) => event.type === "partial_frame");
    const summary = traces.find((event) => event.type === "render_summary");

    expect(container.firstElementChild).toBe(wrapper);
    expect(summary?.fast_path).toBe(true);
    expect(partial?.transition).toBe("revision");
    expect(
      (summary?.anomalies as Array<{ kind?: string }> | undefined)?.some(
        (item) => item.kind === "partial_revision",
      ),
    ).toBe(true);
  });

  it("reuses partial source surface when translation arrives in the same finalize frame", () => {
    const R = renderer();
    R.render(container, partialOnlyPayload("Hello"), { overlay: true });
    const partialSurface = container.querySelector('[data-kind="source"]');
    expect(partialSurface).toBeTruthy();

    const traces = collectTrace(container, {
      preset: "stacked",
      compact: false,
      show_source: true,
      show_translations: true,
      style: minimalStyle(),
      lifecycle_state: "completed_only",
      completed_block_visible: true,
      active_partial_text: "",
      visible_items: [
        { kind: "source", text: "Hello", style_slot: "source" },
        {
          kind: "translation",
          text: "Hola",
          style_slot: "translation_1",
          lang: "es",
        },
      ],
    });

    expect(container.querySelector('[data-kind="source"]')).toBe(partialSurface);
    const completed = traces.find(
      (event) => event.type === "completed_frame" && event.kind === "source",
    );
    expect(completed?.finalized_in_place).toBe(true);
    expect(completed?.animated).toBe(false);

    const summary = traces.find((event) => event.type === "render_summary");
    expect(summary?.fast_path).toBe(false);
  });

  it("reuses completed source surface when translation arrives next frame", () => {
    const R = renderer();
    const completedSource = {
      preset: "stacked",
      compact: false,
      show_source: true,
      show_translations: true,
      style: minimalStyle(),
      lifecycle_state: "completed_only",
      completed_block_visible: true,
      active_partial_text: "",
      visible_items: [{ kind: "source", text: "Hello", style_slot: "source" }],
    };

    R.render(container, completedSource, { overlay: true });
    const sourceSurface = container.querySelector('[data-kind="source"]');
    expect(sourceSurface).toBeTruthy();

    const traces = collectTrace(container, {
      ...completedSource,
      visible_items: [
        { kind: "source", text: "Hello", style_slot: "source" },
        {
          kind: "translation",
          text: "Hola",
          style_slot: "translation_1",
          lang: "es",
        },
      ],
    });

    expect(container.querySelector('[data-kind="source"]')).toBe(sourceSurface);
    const completed = traces.find(
      (event) => event.type === "completed_frame" && event.kind === "source",
    );
    expect(completed?.reused_completed_surface).toBe(true);
    expect(completed?.animated).toBe(false);
  });

  it("uses slow path when a translation row changes the render shape", () => {
    const R = renderer();
    const style = minimalStyle();
    const sourceOnly = {
      preset: "stacked",
      compact: false,
      show_source: true,
      show_translations: true,
      style,
      lifecycle_state: "completed_only",
      completed_block_visible: true,
      active_partial_text: "",
      visible_items: [{ kind: "source", text: "Hello", style_slot: "source" }],
    };

    R.render(container, sourceOnly, { overlay: true });
    const wrapper = container.firstElementChild;

    const traces = collectTrace(container, {
      ...sourceOnly,
      visible_items: [
        { kind: "source", text: "Hello", style_slot: "source" },
        {
          kind: "translation",
          text: "Hola",
          style_slot: "translation_1",
          lang: "es",
        },
      ],
    });

    const summary = traces.find((event) => event.type === "render_summary");
    expect(summary?.fast_path).toBe(false);
    expect(container.firstElementChild).not.toBe(wrapper);
  });

  it("disposeRenderContainer clears persisted render state", () => {
    const R = renderer();
    R.render(container, partialOnlyPayload("Hi"), { overlay: true });
    expect(container.__subtitleStyleRenderState).toBeTruthy();
    R.disposeRenderContainer(container);
    expect(container.__subtitleStyleRenderState).toBeUndefined();
    expect(container.childNodes.length).toBe(0);
  });

  it("uses append-only static merge for long overlay partials", () => {
    const R = renderer();
    const threshold = Number(R.OVERLAY_DENSE_PARTIAL_CHARS || 200);
    const base = "x".repeat(threshold);
    const extended = `${base}y`;

    R.render(container, partialOnlyPayload(base), { overlay: true });
    const staticBefore = container.querySelector(".subtitle-fragment-static");
    const wrapper = container.firstElementChild;
    expect(staticBefore).toBeTruthy();
    expect(container.querySelector(".is-dense-partial")).toBeTruthy();

    const traces = collectTrace(container, partialOnlyPayload(extended));
    expect(container.firstElementChild).toBe(wrapper);

    const staticAfter = container.querySelector(".subtitle-fragment-static");
    expect(staticAfter).toBe(staticBefore);
    expect(staticAfter?.textContent).toBe(base);
    expect(container.querySelector(".subtitle-fragment-fresh")?.textContent).toBe("y");

    const partial = traces.find((event) => event.type === "partial_frame");
    expect(partial?.reused_surface).toBe(true);
    const summary = traces.find((event) => event.type === "render_summary");
    expect(summary?.fast_path).toBe(true);
  });

  it("animates small overlay deltas but skips large ASR bursts", () => {
    const R = renderer();
    const style = {
      ...minimalStyle(),
      base: { ...(minimalStyle().base as Record<string, unknown>), effect: "fade" },
    };
    const threshold = Number(R.OVERLAY_DENSE_PARTIAL_CHARS || 200);
    const long = "x".repeat(threshold);

    R.render(container, partialOnlyPayload(long, { style }), { overlay: true });
    R.render(container, partialOnlyPayload(`${long}abc`, { style }), { overlay: true });
    expect(container.querySelector(".subtitle-fragment-fresh")?.className).toContain("effect-fade");

    R.render(
      container,
      partialOnlyPayload(`${long}abc${"z".repeat(20)}`, { style }),
      { overlay: true },
    );
    expect(container.querySelector(".subtitle-fragment-fresh")?.className).toContain("effect-none");
  });

  it("maps expensive overlay fragment filters to fade", () => {
    const R = renderer();
    expect(R.resolveFreshFragmentEffect("glow", { overlay: true }, 3, 10)).toBe("fade");
    expect(R.resolveFreshFragmentEffect("blur_in", { overlay: true }, 3, 10)).toBe("fade");
    expect(R.resolveFreshFragmentEffect("glow", { overlay: false }, 3, 10)).toBe("glow");
    expect(R.resolveFreshFragmentEffect("pulse", { overlay: true }, 3, 10)).toBe("pulse");
    expect(R.resolveFreshFragmentEffect("reveal", { overlay: true }, 3, 10)).toBe("reveal");
  });

  it("marks animated fresh fragments with transform-capable effect classes", () => {
    const style = {
      ...minimalStyle(),
      base: { ...(minimalStyle().base as Record<string, unknown>), effect: "slide_up" },
    };
    renderer().render(container, partialOnlyPayload("Hi", { style }), { overlay: true });
    const fresh = container.querySelector(".subtitle-fragment-fresh");
    expect(fresh?.className).toContain("effect-slide-up");
    expect(fresh?.className).not.toContain("effect-none");
  });

  it("keeps dashboard UI clamp limits when normalizing base style", () => {
    const R = renderer();
    expect(R.normalizeBaseStyle).toBeTypeOf("function");
    const normalized = R.normalizeBaseStyle!({
      shadow_blur_px: 40,
      line_spacing_em: 2.5,
      letter_spacing_em: 0.5,
      stroke_width_px: 4,
      shadow_offset_x_px: -24,
      background_opacity: "0",
      font_size_px: "42",
    });
    expect(normalized.shadow_blur_px).toBe(40);
    expect(normalized.line_spacing_em).toBe(2.5);
    expect(normalized.letter_spacing_em).toBe(0.5);
    expect(normalized.stroke_width_px).toBe(4);
    expect(normalized.shadow_offset_x_px).toBe(-24);
    expect(R.normalizeBaseStyle!({ stroke_width_px: 12 }).stroke_width_px).toBe(4);
    expect(R.normalizeBaseStyle!({ stroke_width_px: 1.25 }).stroke_width_px).toBe(1.3);
    expect(R.normalizeBaseStyle!({ stroke_width_px: 0.1 }).stroke_width_px).toBe(0.1);
    expect(normalized.background_opacity).toBe(0);
    expect(normalized.font_size_px).toBe(42);
  });

  it("builds CSS vars for stroke, hard shadow, and plate metrics", () => {
    const R = renderer();
    expect(R.buildCssVariables).toBeTypeOf("function");
    const vars = R.buildCssVariables!(
      {
        font_family: "Inter, sans-serif",
        font_size_px: 32,
        font_weight: 600,
        fill_color: "#ffffff",
        stroke_color: "#0b0d12",
        stroke_width_px: 1.2,
        shadow_color: "#000000",
        shadow_blur_px: 0,
        shadow_offset_x_px: 2,
        shadow_offset_y_px: 3,
        background_color: "#12151c",
        background_opacity: 90,
        background_radius_px: 8,
        background_padding_x_px: 16,
        background_padding_y_px: 6,
        line_spacing_em: 1.2,
        letter_spacing_em: 0.015,
        text_align: "left",
      },
      1,
    );
    expect(vars["--subtitle-stroke"]).toBe("#0b0d12");
    // Visible outline is a text-shadow ring (OBS-reliable); webkit stroke is a light assist.
    expect(vars["--subtitle-shadow"]).toContain("1.20px");
    expect(vars["--subtitle-shadow"]).toContain("#0b0d12");
    expect(vars["--subtitle-shadow"]).toMatch(/2px 3px 0px rgba\(/);
    expect(vars["--subtitle-letter-spacing"]).toBe("0.015em");
    expect(vars["--subtitle-text-align"]).toBe("left");
    expect(vars["--subtitle-radius"]).toBe("8px");
    expect(vars["--subtitle-background"]).toContain("rgba");

    expect(R.buildOutlineTextShadow?.(2, "#000000")).toContain("2.00px");
    expect(R.buildOutlineTextShadow?.(0, "#000000")).toBe("");

    const noStroke = R.buildCssVariables!({
      font_family: "Inter",
      font_size_px: 30,
      font_weight: 700,
      fill_color: "#ffffff",
      stroke_color: "#000000",
      stroke_width_px: 0,
      shadow_color: "#000000",
      shadow_blur_px: 0,
      shadow_offset_x_px: 0,
      shadow_offset_y_px: 0,
      background_color: "#000000",
      background_opacity: 0,
      background_radius_px: 0,
      background_padding_x_px: 0,
      background_padding_y_px: 0,
      line_spacing_em: 1.15,
      letter_spacing_em: 0,
      text_align: "center",
    });
    expect(noStroke["--subtitle-stroke"]).toBe("transparent");
    expect(noStroke["--subtitle-shadow"]).toBe("none");
  });

  it("applies slot text_align and base line_gap on render", () => {
    const style = {
      preset: "clean_default",
      base: {
        ...(minimalStyle().base as Record<string, unknown>),
        text_align: "center",
        line_gap_px: 14,
      },
      line_slots: {
        source: { enabled: true, text_align: "left" },
      },
    };
    const effective = renderer().resolveEffectiveStyle?.(style, {});
    expect(effective?.container).toMatchObject({ text_align: "center", line_gap_px: 14 });
    expect((effective?.line_slots as Record<string, Record<string, unknown>>)?.source?.text_align).toBe(
      "left",
    );

    renderer().render(
      container,
      {
        preset: "single",
        compact: false,
        lifecycle_state: "completed_only",
        completed_block_visible: true,
        active_partial_text: "",
        show_source: true,
        show_translations: false,
        visible_items: [{ kind: "source", text: "Hello", style_slot: "source" }],
        style: effective || style,
      },
      { overlay: true },
    );
    const stage = container.querySelector(".subtitle-stage") as HTMLElement | null;
    const row = container.querySelector(".subtitle-line") as HTMLElement | null;
    expect(stage?.style.getPropertyValue("--subtitle-line-gap")).toBe("14px");
    expect(row?.style.getPropertyValue("--subtitle-text-align")).toBe("left");
  });
});
