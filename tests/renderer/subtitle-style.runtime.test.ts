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
});
