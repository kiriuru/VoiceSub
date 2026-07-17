<script lang="ts">
  import { applyLatencyPreset, type LocalAsrConfig } from "../lib/local-asr-api";

  interface Props {
    config: LocalAsrConfig;
    busy: boolean;
    tr: (key: string, vars?: Record<string, string | number>) => string;
    onSave: () => void | Promise<void>;
  }

  let { config, busy, tr, onSave }: Props = $props();

  let advancedOpen = $state(false);

  function onPresetChange() {
    Object.assign(config, applyLatencyPreset(config, config.realtime.latencyPreset));
  }
</script>

<section class="surface-card realtime-panel">
  <div class="section-heading section-heading--stacked">
    <h2>{tr("local_asr.realtime.title")}</h2>
    <p class="muted muted--flush">{tr("local_asr.realtime.note")}</p>
  </div>

  <div class="realtime-form stack">
    <label class="stack-field">
      <span>{tr("local_asr.realtime.latency_preset")}</span>
      <select
        class="control"
        bind:value={config.realtime.latencyPreset}
        disabled={busy}
        onchange={onPresetChange}
      >
        <option value="low">{tr("local_asr.realtime.preset.low")}</option>
        <option value="balanced">{tr("local_asr.realtime.preset.balanced")}</option>
        <option value="quality">{tr("local_asr.realtime.preset.quality")}</option>
      </select>
      <p class="section-note">
        {tr(`local_asr.realtime.preset.hint.${config.realtime.latencyPreset}`)}
      </p>
    </label>

    <label class="checkbox-row">
      <input type="checkbox" bind:checked={config.realtime.streamingDecode} disabled={busy} />
      <span>{tr("local_asr.realtime.streaming_decode")}</span>
    </label>
    <p class="section-note">{tr("local_asr.realtime.streaming_decode.help")}</p>

    <div class="realtime-form__split">
      <label class="stack-field">
        <span>{tr("local_asr.realtime.partial_emit_mode")}</span>
        <select class="control" bind:value={config.realtime.partialEmitMode} disabled={busy}>
          <option value="word_growth">{tr("local_asr.realtime.partial.word_growth")}</option>
          <option value="char_delta">{tr("local_asr.realtime.partial.char_delta")}</option>
        </select>
      </label>

      <label class="stack-field">
        <span>{tr("local_asr.realtime.partial_min_new_words")}</span>
        <input
          class="control"
          type="number"
          min="1"
          max="32"
          bind:value={config.realtime.partialMinNewWords}
          disabled={busy}
        />
      </label>
    </div>
  </div>

  <button
    type="button"
    class="btn btn-ghost btn-sm realtime-form__toggle"
    disabled={busy}
    onclick={() => {
      advancedOpen = !advancedOpen;
    }}
  >
    {advancedOpen ? tr("local_asr.realtime.advanced_hide") : tr("local_asr.realtime.advanced_show")}
  </button>

  {#if advancedOpen}
    <div class="realtime-form stack realtime-form--advanced">
      <div class="realtime-form__split">
        <label class="stack-field">
          <span>{tr("local_asr.realtime.partial_min_delta_chars")}</span>
          <input
            class="control"
            type="number"
            min="0"
            max="256"
            bind:value={config.realtime.partialMinDeltaChars}
            disabled={busy}
          />
        </label>
        <label class="stack-field">
          <span>{tr("local_asr.realtime.partial_coalescing_ms")}</span>
          <input
            class="control"
            type="number"
            min="0"
            max="10000"
            bind:value={config.realtime.partialCoalescingMs}
            disabled={busy}
          />
        </label>
      </div>

      <div class="realtime-form__split">
        <label class="stack-field">
          <span>{tr("local_asr.realtime.decode_interval_ms")}</span>
          <input
            class="control"
            type="number"
            min="0"
            placeholder={tr("local_asr.realtime.preset_default")}
            bind:value={config.realtime.decodeIntervalMs}
            disabled={busy}
          />
        </label>
        <label class="stack-field">
          <span>{tr("local_asr.realtime.window_ms")}</span>
          <input
            class="control"
            type="number"
            min="0"
            placeholder={tr("local_asr.realtime.preset_default")}
            bind:value={config.realtime.windowMs}
            disabled={busy}
          />
        </label>
      </div>

      <label class="stack-field">
        <span>{tr("local_asr.realtime.segment_enqueue_delta_ms")}</span>
        <input
          class="control"
          type="number"
          min="0"
          placeholder={tr("local_asr.realtime.preset_default")}
          bind:value={config.realtime.segmentEnqueueDeltaMs}
          disabled={busy}
        />
      </label>

      <label class="stack-field">
        <span>{tr("local_asr.realtime.first_partial_min_speech_ms")}</span>
        <input
          class="control"
          type="number"
          min="0"
          placeholder={tr("local_asr.realtime.preset_default")}
          bind:value={config.realtime.firstPartialMinSpeechMs}
          disabled={busy}
        />
      </label>

      <p class="realtime-form__group-title">{tr("local_asr.vad.title")}</p>

      <label class="checkbox-row">
        <input type="checkbox" bind:checked={config.vad.enabled} disabled={busy} />
        <span>{tr("local_asr.vad.enabled")}</span>
      </label>

      <div class="realtime-form__split">
        <label class="stack-field">
          <span>{tr("local_asr.vad.vad_mode")}</span>
          <select class="control" bind:value={config.vad.vadMode} disabled={busy}>
            <option value={0}>{tr("local_asr.vad.vad_mode.quality")}</option>
            <option value={1}>{tr("local_asr.vad.vad_mode.low_bitrate")}</option>
            <option value={2}>{tr("local_asr.vad.vad_mode.aggressive")}</option>
            <option value={3}>{tr("local_asr.vad.vad_mode.very_aggressive")}</option>
          </select>
        </label>
        <label class="stack-field">
          <span>{tr("local_asr.vad.min_rms_for_recognition")}</span>
          <input
            class="control"
            type="number"
            min="0"
            max="1"
            step="0.0001"
            bind:value={config.vad.minRmsForRecognition}
            disabled={busy}
          />
        </label>
      </div>

      <label class="checkbox-row">
        <input type="checkbox" bind:checked={config.vad.energyGateEnabled} disabled={busy} />
        <span>{tr("local_asr.vad.energy_gate_enabled")}</span>
      </label>

      <div class="realtime-form__split">
        <label class="stack-field">
          <span>{tr("local_asr.vad.min_voiced_ratio")}</span>
          <input
            class="control"
            type="number"
            min="0"
            max="1"
            step="0.01"
            bind:value={config.vad.minVoicedRatio}
            disabled={busy}
          />
        </label>
        <label class="stack-field">
          <span>{tr("local_asr.vad.partial_emit_interval_ms")}</span>
          <input
            class="control"
            type="number"
            min="0"
            placeholder={tr("local_asr.realtime.preset_default")}
            bind:value={config.vad.partialEmitIntervalMs}
            disabled={busy}
          />
        </label>
      </div>

      <div class="realtime-form__split">
        <label class="stack-field">
          <span>{tr("local_asr.vad.speech_attack_frames")}</span>
          <input
            class="control"
            type="number"
            min="1"
            max="16"
            bind:value={config.vad.speechAttackFrames}
            disabled={busy}
          />
        </label>
        <label class="stack-field">
          <span>{tr("local_asr.vad.speech_preroll_frames")}</span>
          <input
            class="control"
            type="number"
            min="0"
            max="32"
            bind:value={config.vad.speechPrerollFrames}
            disabled={busy}
          />
        </label>
      </div>

      <div class="realtime-form__split">
        <label class="stack-field">
          <span>{tr("local_asr.vad.min_speech_ms")}</span>
          <input class="control" type="number" min="0" bind:value={config.vad.minSpeechMs} disabled={busy} />
        </label>
        <label class="stack-field">
          <span>{tr("local_asr.vad.min_silence_ms")}</span>
          <input class="control" type="number" min="0" bind:value={config.vad.minSilenceMs} disabled={busy} />
        </label>
      </div>

      <div class="realtime-form__split">
        <label class="stack-field">
          <span>{tr("local_asr.vad.silence_hold_ms")}</span>
          <input class="control" type="number" min="0" bind:value={config.vad.silenceHoldMs} disabled={busy} />
        </label>
        <label class="stack-field">
          <span>{tr("local_asr.vad.max_segment_ms")}</span>
          <input class="control" type="number" min="500" bind:value={config.vad.maxSegmentMs} disabled={busy} />
        </label>
      </div>

      <p class="realtime-form__group-title">{tr("local_asr.recognition.title")}</p>

      <div class="realtime-form__split">
        <label class="stack-field">
          <span>{tr("local_asr.recognition.input_gain")}</span>
          <input
            class="control"
            type="number"
            min="0.1"
            max="4"
            step="0.1"
            bind:value={config.recognition.inputGain}
            disabled={busy}
          />
        </label>
        <label class="stack-field">
          <span>{tr("local_asr.recognition.preemphasis_coeff")}</span>
          <input
            class="control"
            type="number"
            min="0"
            max="0.99"
            step="0.01"
            bind:value={config.recognition.preemphasisCoeff}
            disabled={busy || !config.recognition.preemphasisEnabled}
          />
        </label>
      </div>

      <label class="checkbox-row">
        <input type="checkbox" bind:checked={config.recognition.preemphasisEnabled} disabled={busy} />
        <span>{tr("local_asr.recognition.preemphasis_enabled")}</span>
      </label>

      <div class="realtime-form__split">
        <label class="stack-field">
          <span>{tr("local_asr.recognition.noise_gate_threshold")}</span>
          <input
            class="control"
            type="number"
            min="0"
            max="1"
            step="0.001"
            bind:value={config.recognition.noiseGateThreshold}
            disabled={busy || !config.recognition.noiseGateEnabled}
          />
        </label>
        <label class="stack-field">
          <span>{tr("local_asr.recognition.hallucination_min_chars")}</span>
          <input
            class="control"
            type="number"
            min="0"
            bind:value={config.recognition.hallucinationMinChars}
            disabled={busy}
          />
        </label>
      </div>

      <label class="checkbox-row">
        <input type="checkbox" bind:checked={config.recognition.noiseGateEnabled} disabled={busy} />
        <span>{tr("local_asr.recognition.noise_gate_enabled")}</span>
      </label>

      <label class="checkbox-row">
        <input
          type="checkbox"
          bind:checked={config.recognition.hallucinationFilterEnabled}
          disabled={busy}
        />
        <span>{tr("local_asr.recognition.hallucination_filter")}</span>
      </label>

      <label class="stack-field">
        <span>{tr("local_asr.recognition.hallucination_cooldown_ms")}</span>
        <input
          class="control"
          type="number"
          min="0"
          bind:value={config.recognition.hallucinationCooldownMs}
          disabled={busy}
        />
      </label>
    </div>
  {/if}

  <div class="row-actions">
    <button type="button" class="btn btn-primary" disabled={busy} onclick={() => void onSave()}>
      {tr("local_asr.realtime.save")}
    </button>
  </div>
</section>
