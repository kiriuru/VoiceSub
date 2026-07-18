<script lang="ts">
  import { extractPrimaryFontFamily, replacePrimaryFontFamily } from "../font-catalog";
  import {
    clampStrokeWidthPx,
    STROKE_WIDTH_MAX,
    STROKE_WIDTH_MIN,
    STROKE_WIDTH_STEP,
    toCssColorInput,
  } from "../style-field-utils";

  const EFFECTS = [
    "none",
    "fade",
    "subtle_pop",
    "slide_up",
    "zoom_in",
    "blur_in",
    "glow",
    "pulse",
    "reveal",
  ];
  const TEXT_ALIGNS = ["left", "center", "right"] as const;

  export let tr: (key: string) => string;
  export let fonts: Array<{ family: string; label: string }>;
  export let read: (field: string, fallback?: string | number) => string | number;
  export let write: (field: string, value: string | number) => void;
  export let disabled = false;
  export let allowInheritFont = false;
  /** Field keys to omit (e.g. container-only `line_gap_px` on slot editors). */
  export let hiddenFields: string[] = [];

  function isHidden(field: string): boolean {
    return hiddenFields.includes(field);
  }

  function colorValue(field: string, fallback: string): string {
    return toCssColorInput(String(read(field, fallback) || ""), fallback);
  }

  function numberValue(field: string, fallback: number): number {
    const raw = read(field, fallback);
    const parsed = Number(raw);
    return Number.isFinite(parsed) ? parsed : fallback;
  }
</script>

<div class="style-field-group">
  <label class="stack-field style-field-wide">
    <span>{tr("style.field.font_family")}</span>
    <select
      class="control"
      {disabled}
      value={extractPrimaryFontFamily(String(read("font_family", "") || ""))}
      on:change={(e) => {
        const next = (e.currentTarget as HTMLSelectElement).value;
        const current = String(read("font_family", "") || "");
        // Keep Latin/Cyrillic (and other) fallback faces from the preset stack.
        write("font_family", replacePrimaryFontFamily(current, next));
      }}
    >
      {#if allowInheritFont}
        <option value="">{tr("style.slots.inherit_base")}</option>
      {/if}
      {#each fonts as font}
        <option value={font.family}>{font.label}</option>
      {/each}
    </select>
  </label>

  <div class="style-color-strip" aria-label={tr("style.field.text_color")}>
    <label class="color-field">
      <span>{tr("style.field.text_color")}</span>
      <input
        class="control control-color control-color-compact"
        type="color"
        {disabled}
        value={colorValue("fill_color", "#ffffff")}
        on:input={(e) => write("fill_color", (e.currentTarget as HTMLInputElement).value)}
      />
    </label>
    <label class="color-field">
      <span>{tr("style.field.outline_color")}</span>
      <input
        class="control control-color control-color-compact"
        type="color"
        {disabled}
        value={colorValue("stroke_color", "#000000")}
        on:input={(e) => write("stroke_color", (e.currentTarget as HTMLInputElement).value)}
      />
    </label>
    <label class="color-field">
      <span>{tr("style.field.shadow_color")}</span>
      <input
        class="control control-color control-color-compact"
        type="color"
        {disabled}
        value={colorValue("shadow_color", "#000000")}
        on:input={(e) => write("shadow_color", (e.currentTarget as HTMLInputElement).value)}
      />
    </label>
    <label class="color-field">
      <span>{tr("style.field.background_color")}</span>
      <input
        class="control control-color control-color-compact"
        type="color"
        {disabled}
        value={colorValue("background_color", "#000000")}
        on:input={(e) => write("background_color", (e.currentTarget as HTMLInputElement).value)}
      />
    </label>
  </div>

  <div class="style-select-row">
    <label class="stack-field">
      <span>{tr("style.field.text_align")}</span>
      <select
        class="control"
        {disabled}
        value={String(read("text_align", "center"))}
        on:change={(e) => write("text_align", (e.currentTarget as HTMLSelectElement).value)}
      >
        {#each TEXT_ALIGNS as align}
          <option value={align}>{tr(`style.field.text_align.${align}`)}</option>
        {/each}
      </select>
    </label>
    <label class="stack-field">
      <span>{tr("style.field.effect")}</span>
      <select
        class="control"
        {disabled}
        value={String(read("effect", "none"))}
        on:change={(e) => write("effect", (e.currentTarget as HTMLSelectElement).value)}
      >
        {#each EFFECTS as effect}
          <option value={effect}>{tr(`style.field.effect.${effect}`)}</option>
        {/each}
      </select>
    </label>
  </div>

  <div class="style-metric-grid">
    <label class="stack-field">
      <span>{tr("style.field.font_size")}</span>
      <input
        class="control control-metric"
        type="number"
        min="12"
        max="96"
        {disabled}
        value={numberValue("font_size_px", 30)}
        on:input={(e) => write("font_size_px", Number((e.currentTarget as HTMLInputElement).value))}
      />
    </label>
    <label class="stack-field">
      <span>{tr("style.field.font_weight")}</span>
      <input
        class="control control-metric"
        type="number"
        min="300"
        max="900"
        step="100"
        {disabled}
        value={numberValue("font_weight", 700)}
        on:input={(e) => write("font_weight", Number((e.currentTarget as HTMLInputElement).value))}
      />
    </label>
    {#if !isHidden("line_gap_px")}
      <label class="stack-field">
        <span>{tr("style.field.line_gap")}</span>
        <input
          class="control control-metric"
          type="number"
          min="0"
          max="40"
          step="1"
          {disabled}
          value={numberValue("line_gap_px", 8)}
          on:input={(e) => write("line_gap_px", Number((e.currentTarget as HTMLInputElement).value))}
        />
      </label>
    {/if}
    <label class="stack-field">
      <span>{tr("style.field.line_spacing")}</span>
      <input
        class="control control-metric"
        type="number"
        min="0.8"
        max="2.5"
        step="0.05"
        {disabled}
        value={numberValue("line_spacing_em", 1.15)}
        on:input={(e) => write("line_spacing_em", Number((e.currentTarget as HTMLInputElement).value))}
      />
    </label>
    <label class="stack-field">
      <span>{tr("style.field.letter_spacing")}</span>
      <input
        class="control control-metric"
        type="number"
        min="-0.2"
        max="0.5"
        step="0.01"
        {disabled}
        value={numberValue("letter_spacing_em", 0)}
        on:input={(e) => write("letter_spacing_em", Number((e.currentTarget as HTMLInputElement).value))}
      />
    </label>
    <label class="stack-field">
      <span>{tr("style.field.shadow_blur")}</span>
      <input
        class="control control-metric"
        type="number"
        min="0"
        max="40"
        step="1"
        {disabled}
        value={numberValue("shadow_blur_px", 10)}
        on:input={(e) => write("shadow_blur_px", Number((e.currentTarget as HTMLInputElement).value))}
      />
    </label>
    <label class="stack-field">
      <span>{tr("style.field.outline_width")}</span>
      <input
        class="control control-metric"
        type="number"
        min={STROKE_WIDTH_MIN}
        max={STROKE_WIDTH_MAX}
        step={STROKE_WIDTH_STEP}
        inputmode="decimal"
        {disabled}
        value={clampStrokeWidthPx(numberValue("stroke_width_px", 2))}
        on:input={(e) =>
          write(
            "stroke_width_px",
            clampStrokeWidthPx((e.currentTarget as HTMLInputElement).value),
          )}
      />
    </label>
    <label class="stack-field">
      <span>{tr("style.field.shadow_offset_x")}</span>
      <input
        class="control control-metric"
        type="number"
        min="-24"
        max="24"
        step="1"
        {disabled}
        value={numberValue("shadow_offset_x_px", 0)}
        on:input={(e) => write("shadow_offset_x_px", Number((e.currentTarget as HTMLInputElement).value))}
      />
    </label>
    <label class="stack-field">
      <span>{tr("style.field.shadow_offset_y")}</span>
      <input
        class="control control-metric"
        type="number"
        min="-24"
        max="24"
        step="1"
        {disabled}
        value={numberValue("shadow_offset_y_px", 3)}
        on:input={(e) => write("shadow_offset_y_px", Number((e.currentTarget as HTMLInputElement).value))}
      />
    </label>
    <label class="stack-field">
      <span>{tr("style.field.background_opacity")}</span>
      <input
        class="control control-metric"
        type="number"
        min="0"
        max="100"
        {disabled}
        value={numberValue("background_opacity", 0)}
        on:input={(e) => write("background_opacity", Number((e.currentTarget as HTMLInputElement).value))}
      />
    </label>
    <label class="stack-field">
      <span>{tr("style.field.background_radius")}</span>
      <input
        class="control control-metric"
        type="number"
        min="0"
        max="40"
        step="1"
        {disabled}
        value={numberValue("background_radius_px", 10)}
        on:input={(e) => write("background_radius_px", Number((e.currentTarget as HTMLInputElement).value))}
      />
    </label>
    <label class="stack-field">
      <span>{tr("style.field.background_padding_x")}</span>
      <input
        class="control control-metric"
        type="number"
        min="0"
        max="40"
        step="1"
        {disabled}
        value={numberValue("background_padding_x_px", 12)}
        on:input={(e) =>
          write("background_padding_x_px", Number((e.currentTarget as HTMLInputElement).value))}
      />
    </label>
    <label class="stack-field">
      <span>{tr("style.field.background_padding_y")}</span>
      <input
        class="control control-metric"
        type="number"
        min="0"
        max="24"
        step="1"
        {disabled}
        value={numberValue("background_padding_y_px", 4)}
        on:input={(e) =>
          write("background_padding_y_px", Number((e.currentTarget as HTMLInputElement).value))}
      />
    </label>
  </div>
</div>

<style>
  .style-field-group {
    display: grid;
    gap: var(--space-3);
  }

  .style-field-wide {
    min-width: 0;
  }

  .style-select-row {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-3);
  }

  .style-metric-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--space-3);
    align-items: start;
  }

  .style-metric-grid .stack-field {
    min-width: 0;
  }

  .style-metric-grid .stack-field span {
    font-size: 0.78rem;
    line-height: 1.3;
  }

  .control-metric {
    width: 100%;
    max-width: 6.5rem;
  }

  @media (max-width: 1100px) {
    .style-metric-grid {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }

  @media (max-width: 820px) {
    .style-metric-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .control-metric {
      max-width: 8rem;
    }
  }

  @media (max-width: 520px) {
    .style-select-row,
    .style-metric-grid {
      grid-template-columns: 1fr;
    }

    .control-metric {
      max-width: 100%;
    }
  }
</style>
