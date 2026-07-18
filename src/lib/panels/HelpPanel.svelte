<script lang="ts">
  import { locale, t } from "../i18n";

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);

  const quickStartSteps = [
    "help.quick_start.1",
    "help.quick_start.2",
    "help.quick_start.3",
    "help.quick_start.4",
    "help.quick_start.5",
  ] as const;

  const topics = [
    {
      id: "help-section-recognition",
      eyebrowKey: "help.recognition.eyebrow",
      titleKey: "help.recognition.title",
      bodyKey: "help.recognition.body",
      points: ["help.recognition.browser", "help.recognition.local"],
    },
    {
      id: "help-section-translation",
      eyebrowKey: "help.translation.eyebrow",
      titleKey: "help.translation.title",
      bodyKey: "help.translation.body",
      points: [
        "help.translation.providers",
        "help.translation.keys",
        "help.translation.order",
      ],
    },
    {
      id: "help-section-subtitles",
      eyebrowKey: "help.subtitles.eyebrow",
      titleKey: "help.subtitles.title",
      bodyKey: "help.subtitles.body",
      points: [
        "help.subtitles.source_translation",
        "help.subtitles.ttl",
        "help.subtitles.replace",
      ],
    },
    {
      id: "help-section-style",
      eyebrowKey: "help.style.eyebrow",
      titleKey: "help.style.title",
      bodyKey: "help.style.body",
      points: ["help.style.presets", "help.style.slots", "help.style.fonts"],
    },
    {
      id: "help-section-obs",
      eyebrowKey: "help.obs.eyebrow",
      titleKey: "help.obs.title",
      bodyKey: "help.obs.body",
      points: ["help.obs.overlay", "help.obs.cc", "help.obs.debug"],
    },
    {
      id: "help-section-tools",
      eyebrowKey: "help.tools.eyebrow",
      titleKey: "help.tools.title",
      bodyKey: "help.tools.body",
      points: [
        "help.tools.profiles",
        "help.tools.config",
        "help.tools.diagnostics",
      ],
    },
  ] as const;
</script>

<section class="help-layout bento-root stack" aria-labelledby="help-panel-title">
  <header class="help-hero surface-card panel-padding bento-tile bento-span-full stack">
    <div class="section-heading section-heading--stacked">
      <p class="eyebrow">{tr("help.eyebrow")}</p>
      <h2 id="help-panel-title">{tr("help.title")}</h2>
    </div>
    <p class="muted muted--flush">{tr("help.intro")}</p>
  </header>

  <article
    id="help-section-quick-start"
    class="surface-card panel-padding bento-tile bento-span-full stack panel-section-anchor"
  >
    <div class="section-heading section-heading--stacked">
      <p class="eyebrow">{tr("help.quick_start.eyebrow")}</p>
      <h3>{tr("help.quick_start.title")}</h3>
    </div>
    <ol class="help-checklist">
      {#each quickStartSteps as stepKey, index}
        <li class="help-checklist__item">
          <span class="help-checklist__index" aria-hidden="true">{index + 1}</span>
          <span class="help-checklist__text">{tr(stepKey)}</span>
        </li>
      {/each}
    </ol>
  </article>

  <div class="help-topics bento-grid">
    {#each topics as topic}
      <article
        id={topic.id}
        class="surface-card panel-padding bento-tile stack panel-section-anchor"
      >
        <div class="section-heading section-heading--stacked">
          <p class="eyebrow">{tr(topic.eyebrowKey)}</p>
          <h3>{tr(topic.titleKey)}</h3>
        </div>
        <p class="muted muted--flush">{tr(topic.bodyKey)}</p>
        <ul class="help-points">
          {#each topic.points as pointKey}
            <li>{tr(pointKey)}</li>
          {/each}
        </ul>
      </article>
    {/each}
  </div>
</section>

<style>
  .help-layout {
    gap: var(--bento-gap);
  }

  .help-hero {
    min-height: auto;
  }

  .help-checklist {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--space-2);
  }

  .help-checklist__item {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-3);
    align-items: start;
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-md);
    border: 1px solid var(--glass-border);
    background: color-mix(in srgb, var(--surface-1) 72%, transparent);
  }

  .help-checklist__index {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.75rem;
    height: 1.75rem;
    border-radius: var(--radius-sm, 8px);
    background: var(--primary-container);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 600;
    line-height: 1;
    flex-shrink: 0;
  }

  .help-checklist__text {
    font-size: 14px;
    line-height: 1.5;
    color: var(--text-primary);
  }

  .help-points {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--space-2);
  }

  .help-points li {
    position: relative;
    padding-left: 1rem;
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-secondary);
  }

  .help-points li::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0.55em;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: rgb(var(--ui-accent-rgb, 108 199 255) / 0.75);
  }
</style>
