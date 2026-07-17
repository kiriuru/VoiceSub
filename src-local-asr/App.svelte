<script lang="ts">
  import { onMount } from "svelte";
  import { locale, setLocale, t } from "../src/lib/i18n";
  import SetupModal from "./components/SetupModal.svelte";
  import ModuleStatusDetailsSheet from "./components/ModuleStatusDetailsSheet.svelte";
  import ModuleAlertDialog from "./components/ModuleAlertDialog.svelte";
  import RealtimeTuning from "./components/RealtimeTuning.svelte";
  import SessionOptions from "./components/SessionOptions.svelte";
  import { formatModuleAlertMessage, moduleAlertTitle, type ModuleAlertTone } from "./lib/module-alert";
  import {
    deleteLocalAsrDep,
    deleteLocalAsrModel,
    downloadLocalAsrDep,
    downloadLocalAsrModel,
    fetchLocalAsrStatus,
    fetchLocalAsrTestStatus,
    fetchLocalAsrTransfer,
    cancelLocalAsrTransfer,
    fetchCudaToolkitUrl,
    openExternalHttpsUrl,
    listLocalAsrMicrophones,
    loadLocalAsrConfig,
    loadLocalAsrModel,
    probeLocalAsrProvider,
    refreshLocalAsrDeps,
    saveLocalAsrConfig,
    selectLocalAsrModel,
    startLocalAsrTest,
    stopLocalAsrTest,
    unloadLocalAsrModel,
    defaultLocalAsrModuleConfig,
    type LocalAsrConfig,
    type InputDeviceInfo,
    type LocalAsrModuleStatus,
    type ModelFamily,
    type ModelVariant,
    type TransferProgress,
    type TestBenchSnapshot,
  } from "./lib/local-asr-api";
  import { apiFetch } from "./lib/loopback-api-client";

  function defaultModuleConfig(): LocalAsrConfig {
    return defaultLocalAsrModuleConfig();
  }

  let loc = $derived($locale);
  let tr = $derived((key: string, vars?: Record<string, string | number>) => t(key, vars, loc));

  let status = $state<LocalAsrModuleStatus | null>(null);
  let transfer = $state<TransferProgress | null>(null);
  let testBench = $state<TestBenchSnapshot | null>(null);
  let moduleConfig = $state<LocalAsrConfig>(defaultModuleConfig());
  let selectedMicId = $state("");
  let micDevices = $state<InputDeviceInfo[]>([]);
  let busy = $state(false);
  let transferBusy = $state(false);
  let testBusy = $state(false);
  let alertOpen = $state(false);
  let alertTone = $state<ModuleAlertTone>("error");
  let alertMessage = $state("");
  let lastTransferError = $state("");
  let lastTestBenchError = $state("");
  let toolkitUrl = $state(
    "https://developer.nvidia.com/cuda-13-0-0-download-archive?target_os=Windows&target_arch=x86_64&target_type=exe_local",
  );
  let setupOpen = $state(false);
  let statusDetailsOpen = $state(false);
  let loadingModel = $state(false);
  let cancelTransferBusy = $state(false);
  let showDevTools = $state(false);
  let version = $state("0.6.0");
  let transferPollTimer: ReturnType<typeof setInterval> | null = null;
  let testPollTimer: ReturnType<typeof setInterval> | null = null;

  const DEV_TOOLS_STORAGE_KEY = "voicesub.local-asr.devTools";

  function readShowDevTools(): boolean {
    try {
      return globalThis.localStorage?.getItem(DEV_TOOLS_STORAGE_KEY) === "1";
    } catch {
      return false;
    }
  }

  function writeShowDevTools(enabled: boolean) {
    try {
      globalThis.localStorage?.setItem(DEV_TOOLS_STORAGE_KEY, enabled ? "1" : "0");
    } catch {
      // ignore quota / private mode
    }
  }

  async function loadModuleConfig() {
    const config = await loadLocalAsrConfig();
    moduleConfig = config;
    selectedMicId = config.microphone.deviceId ?? "";
    return config;
  }

  async function reload() {
    status = await fetchLocalAsrStatus();
    moduleConfig.inference.executionProvider = status.executionProvider || moduleConfig.inference.executionProvider;
  }

  let alertTitle = $derived(
    moduleAlertTitle(alertTone, {
      error: tr("local_asr.alert.error_title"),
      warn: tr("local_asr.alert.warn_title"),
      info: tr("local_asr.alert.info_title"),
    }),
  );

  function formatAlertMessage(raw: string): string {
    return formatModuleAlertMessage(raw, tr("local_asr.error.network"));
  }

  function showAlert(tone: ModuleAlertTone, raw: string) {
    const message = formatAlertMessage(raw);
    if (!message) return;
    alertTone = tone;
    alertMessage = message;
    alertOpen = true;
  }

  function showError(raw: string) {
    showAlert("error", raw);
  }

  function showWarning(raw: string) {
    showAlert("warn", raw);
  }

  function closeAlert() {
    alertOpen = false;
  }

  $effect(() => {
    const transferError = transfer?.error?.trim() ?? "";
    if (transfer?.cancelled) {
      lastTransferError = "";
      return;
    }
    if (transferError && transferError !== lastTransferError) {
      lastTransferError = transferError;
      showError(transferError);
    }
    if (!transferError) {
      lastTransferError = "";
    }
  });

  $effect(() => {
    const testError = testBench?.error?.trim() ?? "";
    if (testError && testError !== lastTestBenchError) {
      lastTestBenchError = testError;
      showError(testError);
    }
    if (!testError) {
      lastTestBenchError = "";
    }
  });

  function stopTransferPoll() {
    if (transferPollTimer) {
      clearInterval(transferPollTimer);
      transferPollTimer = null;
    }
  }

  function startTransferPoll() {
    stopTransferPoll();
    transferPollTimer = setInterval(() => {
      void fetchLocalAsrTransfer()
        .then((snapshot) => {
          transfer = snapshot;
        })
        .catch(() => {});
    }, 200);
  }

  function stopTestPoll() {
    if (testPollTimer) {
      clearInterval(testPollTimer);
      testPollTimer = null;
    }
  }

  function startTestPoll() {
    stopTestPoll();
    testPollTimer = setInterval(() => {
      void fetchLocalAsrTestStatus()
        .then((snapshot) => {
          testBench = snapshot;
          if (!snapshot.running) {
            stopTestPoll();
            void fetchLocalAsrStatus().then((next) => {
              status = next;
            });
          }
        })
        .catch(() => {});
    }, 300);
  }

  onMount(() => {
    showDevTools = readShowDevTools();
    const onLocaleChanged = (event: Event) => {
      const detail = (event as CustomEvent<{ locale?: string }>).detail;
      const code = detail?.locale;
      if (code === "en" || code === "ru" || code === "ja" || code === "ko" || code === "zh") {
        setLocale(code);
      }
    };
    window.addEventListener("sst:locale-changed", onLocaleChanged);

    void (async () => {
      try {
        try {
          const versionRes = await apiFetch("/api/version");
          if (versionRes.ok) {
            const body = (await versionRes.json()) as { version?: string };
            if (body.version) version = body.version;
          }
        } catch {
          // keep default
        }
        await reload();
        transfer = await fetchLocalAsrTransfer();
        testBench = await fetchLocalAsrTestStatus();
        toolkitUrl = await fetchCudaToolkitUrl();
        micDevices = await listLocalAsrMicrophones();
        await loadModuleConfig();
      } catch (err) {
        showError(err instanceof Error ? err.message : String(err));
      }
    })();

    return () => {
      window.removeEventListener("sst:locale-changed", onLocaleChanged);
      stopTransferPoll();
      stopTestPoll();
    };
  });

  async function run(action: () => Promise<void>) {
    busy = true;
    try {
      await action();
    } catch (err) {
      showError(err instanceof Error ? err.message : String(err));
    } finally {
      busy = false;
    }
  }

  async function runWithTransfer(action: () => Promise<LocalAsrModuleStatus>) {
    transferBusy = true;
    startTransferPoll();
    try {
      status = await action();
      transfer = await fetchLocalAsrTransfer();
    } catch (err) {
      showError(err instanceof Error ? err.message : String(err));
      transfer = await fetchLocalAsrTransfer().catch(() => transfer);
    } finally {
      stopTransferPoll();
      transferBusy = false;
    }
  }

  async function onRefreshDeps() {
    await run(async () => {
      status = await refreshLocalAsrDeps();
    });
  }

  async function onDownload(kind: "ort_cpu" | "ort_gpu" | "cuda_redist") {
    await runWithTransfer(() => downloadLocalAsrDep(kind));
  }

  async function onDeleteDep(kind: "ort_cpu" | "ort_gpu" | "cuda_redist") {
    await run(async () => {
      status = await deleteLocalAsrDep(kind);
      transfer = await fetchLocalAsrTransfer();
    });
  }

  async function onCancelTransfer() {
    if (cancelTransferBusy) return;
    cancelTransferBusy = true;
    try {
      transfer = await cancelLocalAsrTransfer();
    } catch (err) {
      showError(err instanceof Error ? err.message : String(err));
    } finally {
      cancelTransferBusy = false;
    }
  }

  async function onDownloadModel(variant: ModelVariant, family: ModelFamily) {
    await runWithTransfer(() => downloadLocalAsrModel(variant, family));
  }

  async function onDeleteModel(variant: ModelVariant, family: ModelFamily) {
    await run(async () => {
      status = await deleteLocalAsrModel(variant, family);
      transfer = await fetchLocalAsrTransfer();
    });
  }

  async function onSelectModel(variant: ModelVariant, family: ModelFamily) {
    await run(async () => {
      status = await selectLocalAsrModel(variant, family);
      moduleConfig = await loadLocalAsrConfig();
    });
  }

  async function onSaveProvider() {
    await run(async () => {
      const saved = await saveLocalAsrConfig(moduleConfig);
      moduleConfig = saved;
      await reload();
    });
  }

  async function onSaveRealtime() {
    await run(async () => {
      moduleConfig = await saveLocalAsrConfig(moduleConfig);
    });
  }

  async function onSaveSessionOptions() {
    await run(async () => {
      moduleConfig = await saveLocalAsrConfig(moduleConfig);
      await reload();
    });
  }

  async function onSaveMicrophone() {
    await run(async () => {
      moduleConfig.microphone.deviceId = selectedMicId;
      const saved = await saveLocalAsrConfig(moduleConfig);
      moduleConfig = saved;
      selectedMicId = saved.microphone.deviceId;
    });
  }

  async function onProbe(provider: "cpu" | "cuda") {
    await run(async () => {
      const result = await probeLocalAsrProvider(provider);
      status = result.status;
      if (!result.probe.ok) {
        showWarning(result.probe.message || tr("local_asr.alert.probe_failed"));
        return;
      }
      if (result.probe.fallbackProvider) {
        showWarning(result.probe.message);
      }
    });
  }

  async function onLoadModel() {
    loadingModel = true;
    try {
      const result = await loadLocalAsrModel();
      status = result.status;
      if (!result.load.loaded) {
        showWarning(result.load.message || tr("local_asr.alert.load_failed"));
      }
    } catch (err) {
      showError(err instanceof Error ? err.message : String(err));
    } finally {
      loadingModel = false;
    }
  }

  async function onUnloadModel() {
    await run(async () => {
      status = await unloadLocalAsrModel();
    });
  }

  async function onStartTest() {
    testBusy = true;
    try {
      testBench = await startLocalAsrTest(selectedMicId);
      startTestPoll();
    } catch (err) {
      showError(err instanceof Error ? err.message : String(err));
    } finally {
      testBusy = false;
    }
  }

  async function onStopTest() {
    stopTestPoll();
    testBusy = true;
    try {
      testBench = await stopLocalAsrTest();
      status = await fetchLocalAsrStatus();
    } catch (err) {
      showError(err instanceof Error ? err.message : String(err));
      testBench = await fetchLocalAsrTestStatus().catch(() => testBench);
    } finally {
      testBusy = false;
    }
  }

  function mark(ok: boolean): string {
    return ok ? "✓" : "!";
  }

  function markClass(ok: boolean): string {
    return ok ? "badge-ok" : "badge-bad";
  }

  function optionalMark(value: boolean | null): string {
    if (value === null) return "—";
    return mark(value);
  }

  function optionalMarkClass(value: boolean | null): string {
    if (value === null) return "";
    return markClass(value);
  }

  function epBadge(provider: string): string {
    return provider.toUpperCase();
  }

  function micLabel(device: InputDeviceInfo): string {
    return device.label || tr("local_asr.test.mic_default");
  }

  let selectedMicLabel = $derived(
    micDevices.find((device) => device.id === selectedMicId)?.label
      ?? tr("local_asr.test.mic_default"),
  );

  function phaseLabel(phase: string): string {
    const key = `tools.local_asr.phase.${phase}`;
    const label = tr(key);
    return label === key ? phase : label;
  }

  function testPhaseLabel(phase: string | undefined): string {
    const raw = phase ?? "idle";
    const key = `local_asr.test.phase.${raw}`;
    const label = tr(key);
    return label === key ? raw : label;
  }

  function openSetup() {
    setupOpen = true;
  }

  function closeSetup() {
    setupOpen = false;
  }

  function openStatusDetails() {
    statusDetailsOpen = true;
  }

  function closeStatusDetails() {
    statusDetailsOpen = false;
  }
</script>

<div class="local-asr-root app-shell">
  <header class="local-asr-header">
    <div class="local-asr-header__text">
      <h1>{tr("local_asr.header.title")}</h1>
      <p>{tr("local_asr.header.subtitle")}</p>
    </div>
    <button
      type="button"
      class="setup-gear-btn btn btn-ghost"
      aria-label={tr("local_asr.setup.open")}
      title={tr("local_asr.setup.open")}
      disabled={!status || busy}
      onclick={openSetup}
    >
      <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
        <path
          fill="currentColor"
          d="M12 15.5A3.5 3.5 0 0 1 8.5 12 3.5 3.5 0 0 1 12 8.5a3.5 3.5 0 0 1 3.5 3.5 3.5 3.5 0 0 1-3.5 3.5m7.43-2.53c.04-.32.07-.64.07-.97 0-.33-.03-.66-.07-1l2.11-1.63c.19-.15.24-.42.12-.64l-2-3.46c-.12-.22-.39-.31-.61-.22l-2.49 1c-.52-.4-1.06-.73-1.69-.98l-.37-2.65A.506.506 0 0 0 14 2h-4c-.25 0-.46.18-.5.42l-.37 2.65c-.63.25-1.17.59-1.69.98l-2.49-1c-.22-.09-.49 0-.61.22l-2 3.46c-.13.22-.07.49.12.64L4.57 11c-.04.34-.07.67-.07 1 0 .33.03.65.07.97l-2.11 1.66c-.19.15-.25.42-.12.64l2 3.46c.12.22.39.3.61.22l2.49-1.01c.52.4 1.06.74 1.69.99l.37 2.65c.04.24.25.42.5.42h4c.25 0 .46-.18.5-.42l.37-2.65c.63-.26 1.17-.59 1.69-.99l2.49 1.01c.22.08.49 0 .61-.22l2-3.46c.12-.22.07-.49-.12-.64l-2.11-1.66Z"
        />
      </svg>
    </button>
  </header>

  {#if status}
    <section class="surface-card module-status-hero">
      <div class="module-status-hero__main">
        <div class="module-status-hero__kpi">
          <p class="module-status-hero__eyebrow">{tr("local_asr.status.heading")}</p>
          <h2
            class="module-status-hero__phase"
            class:is-error={status.phase === "error"}
            class:is-ready={status.ready}
          >
            {status.ready ? tr("local_asr.status.ready_yes") : tr("local_asr.status.ready_no")}
          </h2>
          <p class="muted module-status-hero__hint">{phaseLabel(status.phase)}</p>
        </div>
      </div>

      <div class="module-status-hero__actions">
        <button type="button" class="btn btn-ghost btn-sm module-status-hero__details" onclick={openStatusDetails}>
          <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
            <path
              fill="currentColor"
              d="M12 2a10 10 0 1 0 10 10A10 10 0 0 0 12 2m1 15h-2v-6h2zm0-8h-2V7h2Z"
            />
          </svg>
          {tr("runtime.details.open")}
        </button>
      </div>

      {#if status.lastError}
        <p class="error-line module-status-hero__error">{status.lastError}</p>
      {/if}
    </section>

    <section class="surface-card">
      <h2 class="section-title">{tr("local_asr.ep.title")}</h2>
      <div class="ep-row">
        <label><input type="radio" bind:group={moduleConfig.inference.executionProvider} value="cpu" /> {tr("local_asr.ep.cpu")}</label>
        <label><input type="radio" bind:group={moduleConfig.inference.executionProvider} value="cuda" /> {tr("local_asr.ep.cuda")}</label>
      </div>
      <label class="checkbox-row">
        <input type="checkbox" bind:checked={moduleConfig.inference.keepModelLoaded} disabled={busy} />
        <span>{tr("local_asr.session.keep_loaded")}</span>
      </label>
      <label class="checkbox-row">
        <input type="checkbox" bind:checked={moduleConfig.inference.cudaFallbackToCpu} disabled={busy} />
        <span>{tr("local_asr.session.cuda_fallback")}</span>
      </label>
      <div class="row-actions">
        <button class="btn btn-primary" disabled={busy} onclick={() => void onSaveProvider()}>{tr("local_asr.ep.save")}</button>
      </div>
      <p class="status-line section-note">{tr("local_asr.ep.note")}</p>
    </section>

    <RealtimeTuning config={moduleConfig} {busy} {tr} onSave={onSaveRealtime} />

    <section class="surface-card">
      <label class="checkbox-row">
        <input
          type="checkbox"
          checked={showDevTools}
          disabled={busy}
          onchange={(event) => {
            showDevTools = (event.currentTarget as HTMLInputElement).checked;
            writeShowDevTools(showDevTools);
          }}
        />
        <span>{tr("local_asr.dev_tools")}</span>
      </label>
      <p class="status-line section-note">{tr("local_asr.dev_tools.help")}</p>
    </section>

    {#if showDevTools}
      <SessionOptions
        config={moduleConfig}
        {busy}
        ortProfilingActive={status.ortProfilingActive}
        ortProfilingDecodeCount={status.ortProfilingDecodeCount}
        ortProfilingMaxDecodes={status.ortProfilingMaxDecodes}
        ortProfilingStoppedBudget={status.ortProfilingStoppedBudget}
        lastOrtProfilePath={status.lastOrtProfilePath}
        {tr}
        onSave={onSaveSessionOptions}
      />
    {/if}

    <section class="surface-card">
      <h2 class="section-title">{tr("local_asr.inference.title")}</h2>
      <p class="status-line">
        {tr("local_asr.inference.configured_ep")}:
        <span class="ep-badge">{epBadge(status.executionProvider)}</span>
        · {tr("local_asr.inference.active_ep")}:
        <span class="ep-badge">{epBadge(status.activeExecutionProvider)}</span>
        · {tr("local_asr.inference.loaded")}:
        <span class={markClass(status.modelLoaded)}>{mark(status.modelLoaded)}</span>
        {#if status.modelLoadMs != null}
          · {tr("local_asr.inference.load_ms", { ms: status.modelLoadMs })}
        {/if}
      </p>
      <p class="status-line">
        {tr("local_asr.inference.probe_cpu")}:
        <span class={optionalMarkClass(status.probeCpuOk)}>{optionalMark(status.probeCpuOk)}</span>
        · {tr("local_asr.inference.probe_cuda")}:
        <span class={optionalMarkClass(status.probeCudaOk)}>{optionalMark(status.probeCudaOk)}</span>
      </p>
      {#if loadingModel}
        <p class="loading-banner" role="status">{tr("local_asr.inference.loading")}</p>
      {/if}
      <div class="row-actions">
        <button class="btn btn-ghost" disabled={busy || loadingModel || !status.modelInstalled} onclick={() => void onProbe("cpu")}>
          {tr("local_asr.inference.probe_cpu_btn")}
        </button>
        <button class="btn btn-ghost" disabled={busy || loadingModel || !status.modelInstalled || !status.env.cudaDepsReady} onclick={() => void onProbe("cuda")}>
          {tr("local_asr.inference.probe_cuda_btn")}
        </button>
        <button class="btn btn-primary" disabled={busy || loadingModel || transferBusy || !status.modelInstalled || status.modelLoaded} onclick={() => void onLoadModel()}>
          {tr("local_asr.inference.load_model")}
        </button>
        <button disabled={busy || loadingModel || !status.modelLoaded} class="btn btn-ghost" onclick={() => void onUnloadModel()}>
          {tr("local_asr.inference.unload_model")}
        </button>
      </div>
    </section>

    <section class="surface-card">
      <h2 class="section-title">{tr("local_asr.test.title")}</h2>
      <div class="mic-row">
        <label class="mic-row__label" for="local-asr-mic-select">{tr("local_asr.test.mic_select")}</label>
        <select id="local-asr-mic-select" class="control" bind:value={selectedMicId} disabled={busy || testBusy || Boolean(testBench?.running)}>
          {#each micDevices as device (device.id)}
            <option value={device.id}>{micLabel(device)}</option>
          {/each}
        </select>
        <button class="btn btn-ghost" disabled={busy || testBusy || Boolean(testBench?.running)} onclick={() => void onSaveMicrophone()}>
          {tr("local_asr.test.mic_save")}
        </button>
        <button class="btn btn-ghost" disabled={busy || testBusy || Boolean(testBench?.running)} onclick={() => void listLocalAsrMicrophones().then((devices) => { micDevices = devices; })}>
          {tr("local_asr.test.mic_refresh")}
        </button>
      </div>
      <p class="status-line">
        {tr("local_asr.test.mic_active")}: <strong>{selectedMicLabel}</strong>
      </p>
      <p class="status-line">
        {tr("local_asr.test.phase")}: <strong>{testPhaseLabel(testBench?.phase)}</strong>
        {#if testBench?.executionProvider}
          · {tr("local_asr.inference.active_ep")}:
          <span class="ep-badge">{epBadge(testBench.executionProvider)}</span>
        {/if}
        {#if testBench?.modelLabel}
          · {tr("local_asr.test.active_model")}:
          <strong>{testBench.modelLabel}</strong>
        {/if}
      </p>
      {#if testBench?.deviceLabel}
        <p class="status-line">
          {tr("local_asr.test.captured_from")}: <strong>{testBench.deviceLabel}</strong>
          {#if testBench.sampleCount > 0}
            · {tr("local_asr.test.samples", { count: testBench.sampleCount, rate: testBench.sampleRate })}
          {/if}
        </p>
      {/if}
      {#if testBench?.running}
        <p class="status-line">
          {tr("local_asr.test.running")}
          {#if testBench.isSpeech}
            · {tr("local_asr.test.speech_active")}
          {/if}
          {#if testBench.decodeCount > 0}
            · {tr("local_asr.test.decode_count", { count: testBench.decodeCount })}
          {/if}
        </p>
      {/if}
      {#if testBench?.partialTranscript && testBench.running}
        <p class="test-transcript test-transcript--live">{testBench.partialTranscript}</p>
      {/if}
      {#if testBench?.transcript}
        <p class="test-transcript">{testBench.transcript}</p>
      {/if}
      {#if testBench?.captureMs}
        <p class="status-line">
          {tr("local_asr.test.timings", {
            capture: testBench?.captureMs ?? 0,
            decode: testBench?.lastDecodeMs ?? 0,
          })}
        </p>
      {/if}
      {#if showDevTools && testBench?.lastDecodeTiming}
        {@const t = testBench.lastDecodeTiming}
        <p class="status-line">
          {tr("local_asr.test.decode_breakdown", {
            audio_ms: t.audioMs,
            prepare_ms: (t.prepareUs / 1000).toFixed(1),
            preprocess_ms: (t.preprocessUs / 1000).toFixed(1),
            parakeet_ms: (t.parakeetTranscribeUs / 1000).toFixed(1),
            outside_pct: t.outsidePct.toFixed(1),
            parakeet_pct: t.parakeetPct.toFixed(1),
          })}
        </p>
        <p class="status-line section-note">{tr("local_asr.test.decode_breakdown_note")}</p>
      {/if}
      <div class="row-actions">
        <button class="btn btn-primary" disabled={busy || testBusy || !status.modelLoaded || Boolean(testBench?.running)} onclick={() => void onStartTest()}>
          {tr("local_asr.test.start")}
        </button>
        <button disabled={testBusy || !testBench?.running} class="btn btn-ghost" onclick={() => void onStopTest()}>
          {tr("local_asr.test.stop")}
        </button>
      </div>
      <p class="status-line section-note">{tr("local_asr.test.note_streaming")}</p>
    </section>

    <SetupModal
      open={setupOpen}
      busy={busy}
      {transferBusy}
      {cancelTransferBusy}
      {status}
      {transfer}
      onOpenCudaToolkit={() => void openExternalHttpsUrl(toolkitUrl)}
      {tr}
      {mark}
      {markClass}
      onClose={closeSetup}
      onRefreshDeps={onRefreshDeps}
      onDownloadDep={onDownload}
      onDeleteDep={onDeleteDep}
      onDownloadModel={onDownloadModel}
      onDeleteModel={onDeleteModel}
      onSelectModel={onSelectModel}
      onCancelTransfer={onCancelTransfer}
    />

    <ModuleStatusDetailsSheet
      open={statusDetailsOpen}
      {status}
      {tr}
      {mark}
      {markClass}
      {phaseLabel}
      onClose={closeStatusDetails}
    />
  {:else}
    <section class="surface-card">
      <p class="status-line">{tr("local_asr.status.loading")}</p>
    </section>
  {/if}

  <ModuleAlertDialog
    open={alertOpen}
    tone={alertTone}
    title={alertTitle}
    message={alertMessage}
    dismissLabel={tr("local_asr.alert.ok")}
    onClose={closeAlert}
  />

  <footer class="app-footer local-asr-module-footer">
    <span class="app-footer__line">
      VoiceSub <span class="app-footer__version">v{version}</span>
      <span class="app-footer__sep" aria-hidden="true">·</span>
      Powered by Kiriuru
    </span>
  </footer>
</div>
