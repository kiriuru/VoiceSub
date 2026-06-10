<script lang="ts">
  import { locale, t } from "../i18n";
  import { UI_THEME_PRESETS } from "../ui-theme-presets";
  import { applyUiPaletteToDocument } from "../ui-theme-css";
  import type { ConfigPayload } from "../types";

  export let config: ConfigPayload;
  export let onChange: (next: ConfigPayload) => void;

  $: loc = $locale;
  function tr(key: string) {
    loc;
    return t(key);
  }

  $: ui = config.ui || {};
  $: palette = ui.palette || {};

  function detectUiPresetId(): string {
    for (const preset of UI_THEME_PRESETS) {
      if (preset.id === "custom") continue;
      if (preset.theme && ui.theme !== preset.theme) continue;
      if (preset.palette) {
        const matches = Object.entries(preset.palette).every(
          ([key, value]) => String(palette[key] || "").toLowerCase() === value.toLowerCase(),
        );
        if (!matches) continue;
      }
      return preset.id;
    }
    return "custom";
  }

  $: activeUiPreset = detectUiPresetId();

  function applyTheme(theme: "dark" | "light") {
    document.documentElement.dataset.uiTheme = theme;
    onChange({ ...config, ui: { ...ui, theme } });
  }

  function patchPalette(key: string, value: string) {
    const nextPalette = { ...palette, [key]: value };
    applyUiPaletteToDocument(nextPalette);
    onChange({ ...config, ui: { ...ui, palette: nextPalette } });
  }

  function applyUiPreset(presetId: string) {
    const preset = UI_THEME_PRESETS.find((item) => item.id === presetId) || UI_THEME_PRESETS[0];
    if (!preset.theme || !preset.palette) return;
    const nextPalette = { ...palette, ...preset.palette };
    document.documentElement.dataset.uiTheme = preset.theme;
    applyUiPaletteToDocument(nextPalette);
    onChange({
      ...config,
      ui: {
        ...ui,
        theme: preset.theme,
        palette: nextPalette,
      },
    });
  }

  $: if (ui.theme) {
    document.documentElement.dataset.uiTheme = ui.theme === "light" ? "light" : "dark";
  }
</script>

<section class="glass-panel panel-padding stack">
  <div class="section-heading">
    <div>
      <p class="eyebrow">{tr("tab.theme")}</p>
      <h2>{tr("style.ui_theme.title")}</h2>
    </div>
  </div>

  <label class="stack-field">
    <span>{tr("style.ui_theme.preset")}</span>
    <select
      class="control"
      value={activeUiPreset}
      on:change={(e) => applyUiPreset((e.currentTarget as HTMLSelectElement).value)}
    >
      {#each UI_THEME_PRESETS as preset}
        <option value={preset.id}>{tr(preset.labelKey)}</option>
      {/each}
    </select>
  </label>

  <div class="badges">
    <button type="button" class="tab-btn" class:active={ui.theme !== "light"} on:click={() => applyTheme("dark")}>
      {tr("style.ui_theme.mode.dark")}
    </button>
    <button type="button" class="tab-btn" class:active={ui.theme === "light"} on:click={() => applyTheme("light")}>
      {tr("style.ui_theme.mode.light")}
    </button>
  </div>

  <div class="palette-grid">
    <label class="stack-field palette-field">
      <span>{tr("style.ui_theme.palette.accent")}</span>
      <input
        class="control control-color"
        type="color"
        value={String(palette.accent || "#6cc7ff")}
        on:input={(e) => patchPalette("accent", (e.currentTarget as HTMLInputElement).value)}
      />
    </label>
    <label class="stack-field palette-field">
      <span>{tr("style.ui_theme.palette.accent_secondary")}</span>
      <input
        class="control control-color"
        type="color"
        value={String(palette.accent_secondary || "#4fe3ff")}
        on:input={(e) => patchPalette("accent_secondary", (e.currentTarget as HTMLInputElement).value)}
      />
    </label>
    <label class="stack-field palette-field">
      <span>{tr("style.ui_theme.palette.accent_tertiary")}</span>
      <input
        class="control control-color"
        type="color"
        value={String(palette.accent_tertiary || "#7ce3ad")}
        on:input={(e) => patchPalette("accent_tertiary", (e.currentTarget as HTMLInputElement).value)}
      />
    </label>
  </div>

  <div class="theme-preview-stage" style="margin-top: 8px;">
    <div class="theme-preview-atmosphere" aria-hidden="true"></div>
    <div class="theme-preview-card">
      <p class="eyebrow">{tr("style.ui_theme.preview.title")}</p>
      <div class="badges">
        <span class="badge active">{tr("style.ui_theme.preview.primary")}</span>
        <span class="badge">{tr("style.ui_theme.preview.secondary")}</span>
      </div>
      <button type="button" class="btn btn-primary" style="margin-top: 12px;">
        {tr("style.ui_theme.preview.cta")}
      </button>
      <p class="muted dashboard-prose-hint" style="margin-top: 10px;">{tr("style.ui_theme.status")}</p>
    </div>
  </div>
</section>
