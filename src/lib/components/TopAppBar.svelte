<script lang="ts">
  import { locale, t } from "../i18n";
  import type { LocaleCode } from "../types";
  import { UI_LOCALES } from "../constants";
  import ArrowLeft from "lucide-svelte/icons/arrow-left";

  export let title = "";
  export let showBack = false;
  export let onBack: (() => void) | undefined = undefined;
  export let onOpenCommandPalette: () => void = () => {};
  export let onSave: () => void = () => {};
  export let busy = false;
  export let localeCode: LocaleCode = "en";
  export let onLanguageChange: (lang: LocaleCode) => void = () => {};

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);
</script>

<header class="top-app-bar">
  <div class="top-app-bar__leading">
    {#if showBack && onBack}
      <button
        type="button"
        class="top-app-bar__icon-btn"
        aria-label={tr("nav.back")}
        on:click={onBack}
      >
        <ArrowLeft size={20} strokeWidth={1.75} />
      </button>
    {/if}
    <h1 class="top-app-bar__title">{title}</h1>
  </div>

  <div class="top-app-bar__actions">
    <label class="sr-only" for="top-app-bar-locale">{tr("language.label")}</label>
    <select
      id="top-app-bar-locale"
      class="control control-sm"
      value={localeCode}
      on:change={(e) => {
        onLanguageChange((e.currentTarget as HTMLSelectElement).value as LocaleCode);
      }}
    >
      {#each UI_LOCALES as item}
        <option value={item.code}>{tr(item.labelKey)}</option>
      {/each}
    </select>

    <button
      type="button"
      class="top-app-bar__icon-btn"
      title={tr("app.chrome.search")}
      aria-label={tr("app.chrome.search")}
      on:click={onOpenCommandPalette}
    >
      ⌕
    </button>

    <button
      type="button"
      class="btn btn-primary top-app-bar__save"
      disabled={busy}
      on:click={onSave}
    >
      {tr("common.save")}
    </button>
  </div>
</header>

<style>
  .control-sm {
    min-height: 36px;
    padding: 0 calc(var(--space-3) + 1.1em) 0 var(--space-3);
    font-size: 12px;
    max-width: 120px;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
</style>
