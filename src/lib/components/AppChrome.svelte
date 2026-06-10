<script lang="ts">
  import { locale, t } from "../i18n";

  export let version = "0.5.0";

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);

  function openPalette() {
    window.dispatchEvent(new CustomEvent("voicesub:open-command-palette"));
  }
</script>

<header class="app-chrome glass-chrome">
  <div class="app-chrome__brand">
    <span class="app-chrome__mark" aria-hidden="true">◆</span>
    <span class="app-chrome__title">VoiceSub</span>
    <span class="app-chrome__version">v{version}</span>
  </div>

  <button type="button" class="app-chrome__search" on:click={openPalette}>
    <span class="app-chrome__search-icon" aria-hidden="true">⌕</span>
    <span class="app-chrome__search-label">{tr("app.chrome.search")}</span>
    <kbd class="app-chrome__kbd">Ctrl+K</kbd>
  </button>
</header>

<style>
  .app-chrome {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    min-height: var(--chrome-height);
    padding: 0 var(--space-4);
    border-radius: var(--radius-lg);
    margin-bottom: var(--space-1);
  }

  .app-chrome__brand {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  .app-chrome__mark {
    color: rgb(var(--ui-accent-rgb));
    font-size: 14px;
  }

  .app-chrome__title {
    font-size: 14px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .app-chrome__version {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-tertiary);
  }

  .app-chrome__search {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex: 1;
    max-width: 360px;
    min-height: 32px;
    padding: 0 var(--space-3);
    border-radius: var(--radius-md);
    border: 1px solid var(--glass-border);
    background: var(--glass-inset);
    color: var(--text-secondary);
    cursor: pointer;
    transition: border-color var(--duration-fast) var(--ease-out);
  }

  .app-chrome__search:hover {
    border-color: rgb(var(--ui-accent-rgb) / 0.35);
    color: var(--text-primary);
  }

  .app-chrome__search-icon {
    opacity: 0.75;
  }

  .app-chrome__search-label {
    flex: 1;
    text-align: left;
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .app-chrome__kbd {
    padding: 2px 6px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--glass-border);
    background: var(--glass-1);
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-tertiary);
  }

  @media (max-width: 720px) {
    .app-chrome {
      flex-wrap: wrap;
      padding-top: var(--space-2);
      padding-bottom: var(--space-2);
    }

    .app-chrome__search {
      max-width: none;
      width: 100%;
    }
  }
</style>
