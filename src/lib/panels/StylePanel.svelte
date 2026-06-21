<script lang="ts">
  import StyleFieldGroup from "../components/StyleFieldGroup.svelte";
  import SubtitleOutputPreview from "../components/SubtitleOutputPreview.svelte";
  import { locale, t } from "../i18n";
  import { fontOptions } from "../font-catalog";
  import {
    applyPresetToLineSlot,
    applyStylePreset,
    deleteCustomPreset,
    mergeStylePresetCatalog,
    localizedPresetDescription,
    presetKeys,
    presetLabel,
    saveCustomPreset,
    seedSlotOverrideFromBase,
  } from "../style-presets";
  import type { ConfigPayload, FontCatalog, RuntimeStatus, StylePresetCatalog } from "../types";

  const LINE_SLOTS = [
    "source",
    "translation_1",
    "translation_2",
    "translation_3",
    "translation_4",
    "translation_5",
  ] as const;

  export let config: ConfigPayload;
  export let subtitleStylePresets: StylePresetCatalog;
  export let fontCatalog: FontCatalog | null;
  export let runtime: RuntimeStatus | null = null;
  export let overlayPayload: Record<string, unknown> | null = null;
  export let onChange: (next: ConfigPayload) => void;

  let activeSlot: (typeof LINE_SLOTS)[number] = "source";
  let customPresetName = "";
  let slotPresetPick = "";

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);

  $: style = (config.subtitle_style || {}) as Record<string, Record<string, unknown>>;
  $: presetCatalog = mergeStylePresetCatalog(subtitleStylePresets || {}, style);
  $: presetList = presetKeys(presetCatalog);
  $: activePreset = String(style.active_preset || style.preset || "clean_default");
  $: fonts = fontOptions(fontCatalog);
  $: baseStyle = (style.base || style.source || {}) as Record<string, unknown>;
  $: lineSlots = (style.line_slots || {}) as Record<string, Record<string, unknown>>;
  $: slotOverride = (lineSlots[activeSlot] || {}) as Record<string, unknown>;
  $: slotEnabled = Boolean(slotOverride.enabled);
  $: slotHint = slotEnabled
    ? t("style.slot.enabled_hint", { slotLabel: slotLabel(activeSlot) }, loc)
    : t("style.slot.disabled_hint", { slotLabel: slotLabel(activeSlot) }, loc);

  function patchStyle(nextStyle: Record<string, Record<string, unknown>>) {
    onChange({ ...config, subtitle_style: nextStyle });
  }

  function patchBase(field: string, value: string | number) {
    const base = { ...(style.base || style.source || {}), [field]: value };
    patchStyle({ ...style, base, source: { ...(style.source || {}), [field]: value } });
  }

  function readBase(field: string, fallback: string | number = ""): string | number {
    const raw = baseStyle[field];
    if (raw === null || raw === undefined || raw === "") return fallback;
    return raw as string | number;
  }

  function readSlot(field: string, fallback: string | number = ""): string | number {
    const raw = slotOverride[field];
    if (raw === null || raw === undefined || raw === "") {
      if (field === "font_family") return "";
      const inherited = baseStyle[field];
      if (inherited === null || inherited === undefined || inherited === "") return fallback;
      return inherited as string | number;
    }
    return raw as string | number;
  }

  function patchSlotField(field: string, value: string | number) {
    const nextSlots = { ...lineSlots };
    const current = { ...(nextSlots[activeSlot] || {}) };
    const normalized = value === "" ? null : value;
    nextSlots[activeSlot] = {
      ...current,
      enabled: true,
      [field]: normalized,
    };
    patchStyle({ ...style, line_slots: nextSlots });
  }

  function patchSlotEnabled(enabled: boolean) {
    const nextSlots = { ...lineSlots };
    const current = { ...(nextSlots[activeSlot] || {}) };
    if (enabled) {
      nextSlots[activeSlot] = seedSlotOverrideFromBase(current, baseStyle);
    } else {
      nextSlots[activeSlot] = { ...current, enabled: false };
    }
    patchStyle({ ...style, line_slots: nextSlots });
  }

  function applyPresetToSlot() {
    const presetName = slotPresetPick.trim();
    if (!presetName) return;
    patchStyle(applyPresetToLineSlot(style, activeSlot, presetName, presetCatalog));
    slotPresetPick = "";
  }

  function selectPreset(key: string) {
    patchStyle(applyStylePreset(style, key, presetCatalog));
  }

  function handleSaveCustomPreset() {
    const name = customPresetName.trim();
    if (!name) return;
    patchStyle(saveCustomPreset(style, name, presetCatalog));
    customPresetName = "";
  }

  function handleDeleteCustomPreset() {
    const name = customPresetName.trim() || activePreset;
    patchStyle(deleteCustomPreset(style, name));
  }

  function slotLabel(slot: string): string {
    if (slot === "source") return tr("common.source");
    return tr(`obs.output.${slot}`);
  }
</script>

<section class="surface-card panel-padding stack">
  <div class="section-heading">
    <div>
      <p class="eyebrow">{tr("style.eyebrow")}</p>
      <h2>{tr("style.title")}</h2>
      <p class="muted">{tr("style.description")}</p>
    </div>
  </div>

  <SubtitleOutputPreview
    {config}
    {runtime}
    {overlayPayload}
    {subtitleStylePresets}
  />

  <label class="stack-field">
    <span>{tr("style.preset")}</span>
    <select class="control" value={activePreset} on:change={(e) => selectPreset((e.currentTarget as HTMLSelectElement).value)}>
      {#each presetList as key}
        <option value={key}>{presetLabel(key, presetCatalog)}</option>
      {/each}
    </select>
    <p class="muted">{localizedPresetDescription(activePreset, presetCatalog, tr)}</p>
  </label>

  <div class="url-row">
    <input class="control" placeholder={tr("style.custom_name.placeholder")} bind:value={customPresetName} />
    <button type="button" class="btn" on:click={handleSaveCustomPreset}>{tr("common.save")}</button>
    <button type="button" class="btn" on:click={handleDeleteCustomPreset}>{tr("style.custom_preset.delete")}</button>
  </div>

  <div class="style-surface stack">
    <div class="section-heading section-heading--stacked">
      <h3>{tr("style.base.title")}</h3>
      <p class="muted">{tr("style.base.note")}</p>
    </div>
    <StyleFieldGroup {tr} {fonts} read={readBase} write={patchBase} />
  </div>

  <div class="style-surface stack">
    <div class="section-heading section-heading--stacked slot-surface-header">
      <div>
        <h3>{tr("style.slots.title")}</h3>
        <p class="muted">{tr("style.slots.note")}</p>
      </div>
      <label class="stack-field checkbox-row slot-enable-row">
        <input
          type="checkbox"
          checked={slotEnabled}
          on:change={(e) => patchSlotEnabled((e.currentTarget as HTMLInputElement).checked)}
        />
        <span>{tr("style.slots.override")}</span>
      </label>
    </div>

    <div class="badges">
      {#each LINE_SLOTS as slot}
        <button type="button" class="tab-btn" class:active={activeSlot === slot} on:click={() => (activeSlot = slot)}>
          {slotLabel(slot)}
        </button>
      {/each}
    </div>

    <p class="muted">{slotHint}</p>

    <label class="stack-field">
      <span>{tr("style.slots.apply_preset")}</span>
      <select
        class="control"
        bind:value={slotPresetPick}
        disabled={!slotEnabled}
        on:change={applyPresetToSlot}
      >
        <option value="">{tr("style.slot.pick_preset_placeholder")}</option>
        {#each presetList as key}
          <option value={key}>{presetLabel(key, presetCatalog)}</option>
        {/each}
      </select>
    </label>

    {#if slotEnabled}
      <StyleFieldGroup
        {tr}
        {fonts}
        allowInheritFont
        read={readSlot}
        write={patchSlotField}
      />
    {/if}
  </div>
</section>

<style>
  .checkbox-row {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }

  .style-surface {
    padding: var(--space-3);
    border-radius: var(--radius-md);
    border: 1px solid var(--line);
    background: rgb(255 255 255 / 0.02);
  }

  .slot-surface-header {
    align-items: flex-start;
    gap: var(--space-3);
  }

  .slot-enable-row {
    flex-shrink: 0;
    margin: 0;
    white-space: nowrap;
  }

  @media (max-width: 720px) {
    .slot-enable-row {
      white-space: normal;
    }
  }
</style>
