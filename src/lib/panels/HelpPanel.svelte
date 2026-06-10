<script lang="ts">
  import { locale, t } from "../i18n";

  $: loc = $locale;
  function tr(key: string) {
    loc;
    return t(key);
  }
</script>

<section class="glass-panel panel-padding help-prose">
  <div class="section-heading">
    <div>
      <p class="eyebrow">{tr("tab.help")}</p>
      <h2>{tr("help.title")}</h2>
    </div>
  </div>

  <p class="muted">{tr("help.intro")}</p>

  <h3>{tr("help.quick_start.title")}</h3>
  <ul>
    <li>{tr("help.quick_start.1")}</li>
    <li>{tr("help.quick_start.2")}</li>
    <li>{tr("help.quick_start.3")}</li>
    <li>{tr("help.quick_start.4")}</li>
  </ul>

  <h3>{tr("help.obs.title")}</h3>
  <p class="muted">{tr("help.obs.body")}</p>

  <h3>{tr("help.recognition.title")}</h3>
  <p class="muted">{tr("help.recognition.browser")}</p>
</section>
