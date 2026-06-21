import type { StylePresetCatalog } from "./types";

export function normalizePresetKey(name: string): string {
  return name.trim().replace(/\s+/g, "_").toLowerCase();
}

export function mergeStylePresetCatalog(
  catalog: StylePresetCatalog,
  style: Record<string, unknown> | undefined,
): StylePresetCatalog {
  const custom = style?.custom_presets;
  if (!custom || typeof custom !== "object") {
    return catalog || {};
  }
  return { ...(catalog || {}), ...(custom as StylePresetCatalog) };
}

export function presetKeys(catalog: StylePresetCatalog): string[] {
  return Object.keys(catalog || {}).sort();
}

export function applyStylePreset(
  style: Record<string, Record<string, unknown>>,
  presetKey: string,
  catalog: StylePresetCatalog,
): Record<string, Record<string, unknown>> {
  const preset = catalog[presetKey];
  if (!preset || typeof preset !== "object") return style;

  const base = {
    ...(typeof style.base === "object" && style.base ? style.base : {}),
    ...(typeof preset.base === "object" && preset.base ? (preset.base as Record<string, unknown>) : {}),
  };

  const lineSlots =
    typeof preset.line_slots === "object" && preset.line_slots
      ? structuredClone(preset.line_slots as Record<string, Record<string, unknown>>)
      : {};

  return {
    ...style,
    active_preset: presetKey,
    preset: presetKey,
    base,
    line_slots: lineSlots,
    source: { ...(style.source || {}), ...base },
    translation_1: { ...(style.translation_1 || style.translation || {}), ...base },
  } as unknown as Record<string, Record<string, unknown>>;
}

export function saveCustomPreset(
  style: Record<string, Record<string, unknown>>,
  name: string,
  catalog: StylePresetCatalog,
): Record<string, Record<string, unknown>> {
  const key = normalizePresetKey(name);
  if (!key) return style;

  const customPresets = {
    ...(typeof style.custom_presets === "object" && style.custom_presets
      ? (style.custom_presets as Record<string, unknown>)
      : {}),
  };

  customPresets[key] = {
    preset: key,
    label: name.trim(),
    description: "User-created local subtitle style.",
    built_in: false,
    base: style.base || {},
    line_slots: style.line_slots || {},
  };

  return {
    ...style,
    custom_presets: customPresets,
    active_preset: key,
    preset: key,
  } as unknown as Record<string, Record<string, unknown>>;
}

export function deleteCustomPreset(
  style: Record<string, Record<string, unknown>>,
  name: string,
): Record<string, Record<string, unknown>> {
  const key = normalizePresetKey(name);
  if (!key || typeof style.custom_presets !== "object" || !style.custom_presets) return style;
  const customPresets = { ...(style.custom_presets as Record<string, unknown>) };
  delete customPresets[key];
  const active = String(style.active_preset || style.preset || "clean_default");
  const nextActive = active === key ? "clean_default" : active;
  return {
    ...style,
    custom_presets: customPresets,
    active_preset: nextActive,
    preset: nextActive,
  } as unknown as Record<string, Record<string, unknown>>;
}

export function presetLabel(key: string, catalog: StylePresetCatalog): string {
  const entry = catalog[key];
  if (entry && typeof entry === "object" && typeof entry.label === "string") {
    return entry.label;
  }
  return key.replaceAll("_", " ");
}

function presetDescription(key: string, catalog: StylePresetCatalog): string {
  const entry = catalog[key];
  if (entry && typeof entry === "object" && typeof entry.description === "string") {
    return entry.description;
  }
  return "";
}

export function localizedPresetDescription(
  key: string,
  catalog: StylePresetCatalog,
  tr: (i18nKey: string) => string,
): string {
  const entry = catalog[key];
  const isBuiltIn =
    entry && typeof entry === "object" && (entry as { built_in?: boolean }).built_in === true;
  if (isBuiltIn) {
    const i18nKey = `style.preset.desc.${key}`;
    const translated = tr(i18nKey);
    if (translated !== i18nKey) {
      return translated;
    }
  }
  const raw = presetDescription(key, catalog);
  if (raw === "User-created local subtitle style.") {
    const custom = tr("style.preset.custom_description");
    if (custom !== "style.preset.custom_description") {
      return custom;
    }
  }
  return raw || tr("style.preset.default_description");
}

export function buildStyleFromPreset(
  catalog: StylePresetCatalog,
  presetKey: string,
): Record<string, unknown> {
  const renderer =
    typeof window !== "undefined"
      ? (
          window as Window & {
            SubtitleStyleRenderer?: {
              buildStyleFromPreset: (presets: unknown, name: string) => Record<string, unknown>;
            };
          }
        ).SubtitleStyleRenderer
      : undefined;
  if (renderer?.buildStyleFromPreset) {
    return renderer.buildStyleFromPreset(catalog, presetKey);
  }
  const preset = catalog[presetKey];
  if (!preset || typeof preset !== "object") return {};
  return {
    preset: presetKey,
    base: typeof preset.base === "object" && preset.base ? { ...preset.base } : {},
    line_slots:
      typeof preset.line_slots === "object" && preset.line_slots ? { ...preset.line_slots } : {},
  };
}

export function seedSlotOverrideFromBase(
  slot: Record<string, unknown>,
  base: Record<string, unknown>,
): Record<string, unknown> {
  const next: Record<string, unknown> = { ...slot, enabled: true };
  Object.entries(base).forEach(([key, value]) => {
    if (key === "effect" && value == null) return;
    if (next[key] === null || next[key] === undefined || next[key] === "") {
      next[key] = value;
    }
  });
  return next;
}

export function applyPresetToLineSlot(
  style: Record<string, Record<string, unknown>>,
  slotName: string,
  presetKey: string,
  catalog: StylePresetCatalog,
): Record<string, Record<string, unknown>> {
  const presetStyle = buildStyleFromPreset(catalog, presetKey);
  const sourceBase = (presetStyle.base || {}) as Record<string, unknown>;
  if (!Object.keys(sourceBase).length) return style;

  const lineSlots = {
    ...((style.line_slots as Record<string, Record<string, unknown>>) || {}),
  };
  const current = { ...(lineSlots[slotName] || {}) };
  const next: Record<string, unknown> = { ...current, enabled: true };
  Object.entries(sourceBase).forEach(([key, value]) => {
    next[key] = value === null || value === undefined ? null : String(value);
  });
  lineSlots[slotName] = next as Record<string, unknown>;
  return { ...style, line_slots: lineSlots };
}
