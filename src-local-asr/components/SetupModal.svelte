<script lang="ts">
  import type {
    DepDownloadKind,
    LocalAsrModuleStatus,
    ModelCatalogEntry,
    ModelFamily,
    ModelVariant,
    TransferProgress,
  } from "../lib/local-asr-api";
  import {
    defaultModelCatalog,
    defaultVariantForFamily,
    formatTransferBytes,
    formatTransferPercent,
    formatTransferSpeed,
    parseModelFamily,
    transferDisplayPercent,
    transferEffectiveTotalBytes,
  } from "../lib/local-asr-api";

  interface Props {
    open: boolean;
    busy: boolean;
    transferBusy: boolean;
    cancelTransferBusy: boolean;
    status: LocalAsrModuleStatus;
    transfer: TransferProgress | null;
    tr: (key: string, vars?: Record<string, string | number>) => string;
    mark: (ok: boolean) => string;
    markClass: (ok: boolean) => string;
    onClose: () => void;
    onOpenCudaToolkit: () => void | Promise<void>;
    onRefreshDeps: () => void | Promise<void>;
    onDownloadDep: (kind: DepDownloadKind) => void | Promise<void>;
    onDeleteDep: (kind: DepDownloadKind) => void | Promise<void>;
    onDownloadModel: (variant: ModelVariant, family: ModelFamily) => void | Promise<void>;
    onDeleteModel: (variant: ModelVariant, family: ModelFamily) => void | Promise<void>;
    onSelectModel: (variant: ModelVariant, family: ModelFamily) => void | Promise<void>;
    onCancelTransfer: () => void | Promise<void>;
  }

  let {
    open,
    busy,
    transferBusy,
    cancelTransferBusy,
    status,
    transfer,
    tr,
    mark,
    markClass,
    onClose,
    onOpenCudaToolkit,
    onRefreshDeps,
    onDownloadDep,
    onDeleteDep,
    onDownloadModel,
    onDeleteModel,
    onSelectModel,
    onCancelTransfer,
  }: Props = $props();

  let selectedModelFamily = $state<ModelFamily>("parakeet_tdt");
  let selectedModelVariant = $state<ModelVariant>("int8");
  let wasOpen = $state(false);

  $effect(() => {
    if (open && !wasOpen) {
      selectedModelFamily = parseModelFamily(status.activeModelFamily || "parakeet_tdt");
      selectedModelVariant =
        status.activeModelVariant || defaultVariantForFamily(selectedModelFamily);
    }
    wasOpen = open;
  });

  let displayModels = $derived.by(() => {
    const fromStatus = status.models.filter(
      (model) => parseModelFamily(model.family ?? selectedModelFamily) === selectedModelFamily,
    );
    if (fromStatus.length > 0) {
      return fromStatus;
    }
    return defaultModelCatalog(selectedModelFamily);
  });

  let activeTransfer = $derived(transfer?.active ? transfer : null);

  function modelVariantLabel(variant: ModelVariant): string {
    const key = `local_asr.model.variant.${variant}`;
    const translated = tr(key);
    return translated === key ? variant : translated;
  }

  function modelAuthorLabel(author: string): string {
    if (!author) return "";
    return tr("local_asr.model.author_label", { author });
  }

  function modelOptionLabel(model: ModelCatalogEntry): string {
    const base = `${modelVariantLabel(model.variant)} · ${modelAuthorLabel(model.sourceAuthor)}`;
    const size = tr("local_asr.system.size_mb", { size: model.sizeMb });
    if (model.installed) {
      return `${base} · ${size} · ${tr("local_asr.model.installed_badge")}`;
    }
    return `${base} · ${size}`;
  }

  let selectedModelApplied = $derived(
    selectedModelVariant === status.activeModelVariant &&
      selectedModelFamily === parseModelFamily(status.activeModelFamily || "parakeet_tdt"),
  );

  function phaseLabel(phase: TransferProgress["phase"]): string {
    if (phase === "extracting") return tr("local_asr.transfer.extracting");
    if (phase === "finalizing") return tr("local_asr.transfer.finalizing");
    return tr("local_asr.transfer.downloading");
  }

  function transferProgressText(item: TransferProgress): string {
    const received = formatTransferBytes(item.receivedBytes);
    const speed = formatTransferSpeed(item.speedBps);
    const percent = transferDisplayPercent(item);
    const percentText = percent != null ? formatTransferPercent(percent) : null;
    const totalBytes = transferEffectiveTotalBytes(item);
    if (totalBytes != null) {
      return tr("local_asr.transfer.progress", {
        received,
        total: formatTransferBytes(totalBytes),
        speed,
        percent: percentText ?? "",
      });
    }
    return tr("local_asr.transfer.unknown_total", { received, speed });
  }

  function transferBarWidth(item: TransferProgress): string | null {
    const percent = transferDisplayPercent(item);
    return percent != null ? `${percent}%` : null;
  }

  function transferBarIndeterminate(item: TransferProgress): boolean {
    return item.phase === "downloading" && transferDisplayPercent(item) == null && item.receivedBytes > 0;
  }

  function isTransferTarget(target: string): boolean {
    return Boolean(activeTransfer && activeTransfer.target === target);
  }

  function onBackdropClick() {
    onClose();
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") onClose();
  }
</script>

{#if open}
  <button
    type="button"
    class="setup-modal-backdrop"
    aria-label={tr("local_asr.setup.close")}
    tabindex="-1"
    onclick={onBackdropClick}
  ></button>
  <div
    class="setup-modal"
    role="dialog"
    aria-modal="true"
    aria-labelledby="local-asr-setup-title"
    tabindex="-1"
    onclick={(event) => event.stopPropagation()}
    onkeydown={onKeydown}
  >
    <header class="setup-modal__header">
      <h2 id="local-asr-setup-title">{tr("local_asr.setup.title")}</h2>
      <button
        type="button"
        class="top-app-bar__icon-btn"
        aria-label={tr("local_asr.setup.close")}
        onclick={onClose}
      >
        <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
          <path
            fill="currentColor"
            d="M18.3 5.71a1 1 0 0 0-1.41 0L12 10.59 7.11 5.7A1 1 0 0 0 5.7 7.11L10.59 12l-4.89 4.89a1 1 0 1 0 1.41 1.42L12 13.41l4.89 4.89a1 1 0 0 0 1.42-1.41L13.41 12l4.89-4.89a1 1 0 0 0-.01-1.4Z"
          />
        </svg>
      </button>
    </header>

    {#if activeTransfer}
      <section class="transfer-panel" aria-live="polite">
        <div class="transfer-panel__head">
          <div class="transfer-panel__title">
            <strong>{activeTransfer.label}</strong>
            <span>{phaseLabel(activeTransfer.phase)}</span>
          </div>
          <button
            type="button"
            class="btn btn-ghost transfer-panel__cancel"
            disabled={cancelTransferBusy}
            onclick={() => void onCancelTransfer()}
          >
            {tr("local_asr.transfer.cancel")}
          </button>
        </div>
        <div
          class="transfer-panel__bar"
          class:transfer-panel__bar--indeterminate={activeTransfer && transferBarIndeterminate(activeTransfer)}
          role="progressbar"
          aria-valuemin="0"
          aria-valuemax="100"
          aria-valuenow={activeTransfer ? transferDisplayPercent(activeTransfer) ?? undefined : undefined}
        >
          <div
            class="transfer-panel__fill"
            style:width={activeTransfer ? transferBarWidth(activeTransfer) : null}
          ></div>
        </div>
        <p class="transfer-panel__meta">{transferProgressText(activeTransfer)}</p>
      </section>
    {/if}

    <section class="setup-modal__section">
      <h3>{tr("local_asr.system.title")}</h3>
      <table class="deps-table">
        <thead>
          <tr>
            <th>{tr("local_asr.system.col.component")}</th>
            <th>{tr("local_asr.system.col.status")}</th>
            <th>{tr("local_asr.system.col.note")}</th>
            <th>{tr("local_asr.system.col.actions")}</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td>{tr("local_asr.system.vcruntime")}</td>
            <td class={markClass(status.env.vcruntime.ok)}>{mark(status.env.vcruntime.ok)}</td>
            <td>{status.env.vcruntime.missing.join(", ") || tr("local_asr.system.ok")}</td>
            <td></td>
          </tr>
          <tr class:is-active={isTransferTarget("ort_cpu")}>
            <td>{tr("local_asr.system.ort_cpu")}</td>
            <td class={markClass(status.env.ortCpu.ok)}>{mark(status.env.ortCpu.ok)}</td>
            <td>{tr("local_asr.system.size_mb", { size: status.env.ortCpu.downloadMb })}</td>
            <td>
              <div class="row-inline-actions">
                <button
                  type="button"
                  class="btn btn-primary btn-sm"
                  disabled={busy || transferBusy || status.env.ortCpu.ok}
                  onclick={() => void onDownloadDep("ort_cpu")}
                >
                  {tr("local_asr.system.download")}
                </button>
                <button
                  type="button"
                  class="icon-btn danger"
                  disabled={busy || !status.env.ortCpu.ok}
                  aria-label={tr("local_asr.system.delete")}
                  title={tr("local_asr.system.delete")}
                  onclick={() => void onDeleteDep("ort_cpu")}
                >
                  <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                    <path
                      fill="currentColor"
                      d="M9 3h6l1 2h4v2H4V5h4l1-2m1 4h10v14H10V7m2 3v9h2v-9h-2m4 0v9h2v-9h-2Z"
                    />
                  </svg>
                </button>
              </div>
            </td>
          </tr>
          <tr class:is-active={isTransferTarget("ort_gpu")}>
            <td>{tr("local_asr.system.ort_gpu")}</td>
            <td class={markClass(status.env.ortGpu.ok)}>{mark(status.env.ortGpu.ok)}</td>
            <td>{tr("local_asr.system.size_mb", { size: status.env.ortGpu.downloadMb })}</td>
            <td>
              <div class="row-inline-actions">
                <button
                  type="button"
                  class="btn btn-primary btn-sm"
                  disabled={busy || transferBusy || status.env.ortGpu.ok}
                  onclick={() => void onDownloadDep("ort_gpu")}
                >
                  {tr("local_asr.system.download")}
                </button>
                <button
                  type="button"
                  class="icon-btn danger"
                  disabled={busy || !status.env.ortGpu.ok}
                  aria-label={tr("local_asr.system.delete")}
                  title={tr("local_asr.system.delete")}
                  onclick={() => void onDeleteDep("ort_gpu")}
                >
                  <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                    <path
                      fill="currentColor"
                      d="M9 3h6l1 2h4v2H4V5h4l1-2m1 4h10v14H10V7m2 3v9h2v-9h-2m4 0v9h2v-9h-2Z"
                    />
                  </svg>
                </button>
              </div>
            </td>
          </tr>
          <tr class:is-active={isTransferTarget("cuda_redist")}>
            <td>{tr("local_asr.system.cuda_redist")}</td>
            <td class={markClass(status.env.cudaRedist.ok)}>{mark(status.env.cudaRedist.ok)}</td>
            <td>{tr("local_asr.system.size_mb", { size: status.env.cudaRedist.downloadMb })}</td>
            <td>
              <div class="row-inline-actions">
                <button
                  type="button"
                  class="btn btn-primary btn-sm"
                  disabled={busy || transferBusy || status.env.cudaRedist.ok}
                  onclick={() => void onDownloadDep("cuda_redist")}
                >
                  {tr("local_asr.system.download")}
                </button>
                <button
                  type="button"
                  class="icon-btn danger"
                  disabled={busy || !status.env.cudaRedist.ok}
                  aria-label={tr("local_asr.system.delete")}
                  title={tr("local_asr.system.delete")}
                  onclick={() => void onDeleteDep("cuda_redist")}
                >
                  <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                    <path
                      fill="currentColor"
                      d="M9 3h6l1 2h4v2H4V5h4l1-2m1 4h10v14H10V7m2 3v9h2v-9h-2m4 0v9h2v-9h-2Z"
                    />
                  </svg>
                </button>
              </div>
            </td>
          </tr>
          <tr>
            <td>{tr("local_asr.system.cuda_toolkit")}</td>
            <td class={markClass(status.env.cudaToolkit.ok)}>{mark(status.env.cudaToolkit.ok)}</td>
            <td>{status.env.cudaToolkit.message || status.env.cudaToolkit.version || ""}</td>
            <td>
              <button type="button" class="btn btn-ghost btn-sm" onclick={() => void onOpenCudaToolkit()}>
                {tr("local_asr.system.cuda_toolkit_link")}
              </button>
            </td>
          </tr>
        </tbody>
      </table>

      <div class="row-actions">
        <button type="button" class="btn btn-ghost" disabled={busy} onclick={() => void onRefreshDeps()}>
          {tr("local_asr.system.recheck")}
        </button>
      </div>
    </section>

    <section class="setup-modal__section">
      <h3>{tr("local_asr.model.title")}</h3>
      <div class="model-select-row">
        <label class="model-select-row__label" for="local-asr-model-variant">
          {tr("local_asr.model.select")}
        </label>
        <select
          id="local-asr-model-variant"
          class="control model-select-row__select"
          bind:value={selectedModelVariant}
          disabled={busy}
        >
          {#each displayModels as model (`${model.family ?? selectedModelFamily}:${model.variant}`)}
            <option value={model.variant}>{modelOptionLabel(model)}</option>
          {/each}
        </select>
        <button
          type="button"
          class="btn btn-primary"
          disabled={busy || selectedModelApplied}
          onclick={() => void onSelectModel(selectedModelVariant, selectedModelFamily)}
        >
          {tr("local_asr.model.apply")}
        </button>
      </div>
      <p class="status-line section-note">{tr("local_asr.model.select_note")}</p>
      <table class="deps-table">
        <thead>
          <tr>
            <th>{tr("local_asr.system.col.component")}</th>
            <th>{tr("local_asr.system.col.status")}</th>
            <th>{tr("local_asr.system.col.note")}</th>
            <th>{tr("local_asr.system.col.actions")}</th>
          </tr>
        </thead>
        <tbody>
          {#each displayModels as model (`${model.family ?? selectedModelFamily}:${model.variant}`)}
            <tr class:is-active={isTransferTarget(`model:${selectedModelFamily}:${model.variant}`)}>
              <td>
                {modelVariantLabel(model.variant)}
                {#if model.sourceAuthor}
                  <span class="model-author-tag">{modelAuthorLabel(model.sourceAuthor)}</span>
                {/if}
              </td>
              <td class={markClass(model.installed)}>{mark(model.installed)}</td>
              <td>
                {tr("local_asr.system.size_mb", { size: model.sizeMb })}
                {#if model.active}
                  · {tr("local_asr.model.active_badge")}
                {/if}
              </td>
              <td>
                <div class="row-inline-actions">
                  <button
                    type="button"
                    class="btn btn-primary btn-sm"
                    disabled={busy || transferBusy || model.installed}
                    onclick={() => void onDownloadModel(model.variant, selectedModelFamily)}
                  >
                    {tr("local_asr.system.download")}
                  </button>
                  <button
                    type="button"
                    class="icon-btn danger"
                    disabled={busy || !model.installed}
                    aria-label={tr("local_asr.model.delete")}
                    title={tr("local_asr.model.delete")}
                    onclick={() => void onDeleteModel(model.variant, selectedModelFamily)}
                  >
                    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                      <path
                        fill="currentColor"
                        d="M9 3h6l1 2h4v2H4V5h4l1-2m1 4h10v14H10V7m2 3v9h2v-9h-2m4 0v9h2v-9h-2Z"
                      />
                    </svg>
                  </button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </section>
  </div>
{/if}
