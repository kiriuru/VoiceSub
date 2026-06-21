(function () {
  if (window.I18n) {
    window.I18n.apply(document);
    document.title = window.I18n.t("document.title.overlay");
    window.addEventListener("sst:locale-changed", () => {
      window.I18n.apply(document);
      document.title = window.I18n.t("document.title.overlay");
    });
  }

  const LIFECYCLE_STATES = new Set([
    "idle",
    "partial_only",
    "completed_only",
    "completed_with_partial",
  ]);
  const OVERLAY_PRESETS = new Set(["single", "dual-line", "stacked"]);

  const params = new URLSearchParams(location.search);
  const compact = params.get("compact") === "1";
  const debugMode = params.get("debug") === "1";
  // Independent toggle for subtitle-effect tracing so the OBS overlay can run
  // in clean debug=1 mode without the per-frame partial chatter, and vice
  // versa. Enable by adding ?debug-subtitles=1 to the overlay URL, or by
  // setting localStorage.sst_debug_subtitles = "1" once (persists across
  // reloads — handy when the overlay URL is locked behind OBS).
  const subtitleDebugFromUrl = params.get("debug-subtitles") === "1";
  let subtitleDebugFromStorage = false;
  try {
    subtitleDebugFromStorage = window.localStorage && window.localStorage.getItem("sst_debug_subtitles") === "1";
  } catch (_error) {
    subtitleDebugFromStorage = false;
  }
  const subtitleDebugMode = subtitleDebugFromUrl || subtitleDebugFromStorage;
  const presetParam = params.get("preset") || "";
  const preset = ["single", "dual-line", "stacked", "compact"].includes(presetParam) ? presetParam : "single";

  const root = document.getElementById("overlay-root");
  const linesContainer = document.getElementById("overlay-lines");

  const overlayState = {
    preset: preset === "compact" ? "stacked" : preset,
    compact: compact || presetParam === "compact",
    completedItems: [],
    activePartialText: "",
    showSource: true,
    showTranslations: true,
    lifecycleState: "idle",
    lastRenderSignature: "",
    lastPayloadStyle: null,
  };

  const runtimeGoneClearDelayMs = 900;
  const staleGuard = window.SstWsStaleGuard?.createWsStaleGuardState?.() || {
    sequenceByType: new Map(),
    timestampByType: new Map(),
  };
  const normalizeWsEventType = window.SstWsStaleGuard?.normalizeWsEventType || ((type) => String(type || "").trim().toLowerCase());
  const isWsEventStale = window.SstWsStaleGuard?.isWsEventStale || (() => false);
  let connectionId = 0;
  let reconnectBackoffMs = 1000;
  const maxReconnectBackoffMs = 10000;
  let reconnectTimer = null;
  let pendingOverlayPayload = null;
  let overlayRenderRafId = 0;
  let overlayRenderTimerId = 0;
  let lastOverlayRenderAt = 0;
  // Cap overlay repaint rate for long partial text (~15 fps). OBS Browser Source
  // at unlimited FPS still composites every DOM change; long stroked lines are costly.
  const OVERLAY_LONG_TEXT_MIN_RENDER_MS = 66;

  function resolveLongTextThreshold() {
    const fromRenderer = window.SubtitleStyleRenderer?.OVERLAY_DENSE_PARTIAL_CHARS;
    return Number.isFinite(fromRenderer) && fromRenderer > 0 ? fromRenderer : 200;
  }

  function maxOverlayPayloadTextLength(payload) {
    if (!payload || typeof payload !== "object") {
      return 0;
    }
    let maxLen = String(payload.active_partial_text || "").length;
    const items = Array.isArray(payload.visible_items) ? payload.visible_items : [];
    items.forEach((item) => {
      const len = String(item?.text || "").length;
      if (len > maxLen) {
        maxLen = len;
      }
    });
    return maxLen;
  }

  function overlayRenderMinIntervalMs(payload) {
    return maxOverlayPayloadTextLength(payload) >= resolveLongTextThreshold()
      ? OVERLAY_LONG_TEXT_MIN_RENDER_MS
      : 0;
  }

  function normalizeOverlayPayload(raw) {
    const current = raw && typeof raw === "object" ? raw : {};
    const rawLifecycle = String(current.lifecycle_state || "idle");
    const lifecycle_state = LIFECYCLE_STATES.has(rawLifecycle) ? rawLifecycle : "idle";
    const rawPreset = String(current.preset || "stacked");
    return {
      preset: OVERLAY_PRESETS.has(rawPreset) ? rawPreset : "stacked",
      compact: current.compact === true,
      completed_block_visible: current.completed_block_visible === true,
      lifecycle_state,
      show_source: current.show_source !== false,
      show_translations: current.show_translations !== false,
      active_partial_text: String(current.active_partial_text || ""),
      max_translation_languages: Number(current.max_translation_languages || 0),
      display_order: Array.isArray(current.display_order) ? current.display_order : [],
      visible_items: (Array.isArray(current.visible_items) ? current.visible_items : []).map((item) => ({
        kind: String(item?.kind || "source"),
        text: String(item?.text || ""),
        style_slot: String(item?.style_slot || ""),
        lang: String(item?.lang || ""),
        slot_id: String(item?.slot_id || ""),
        target_lang: String(item?.target_lang || ""),
      })),
      style: current.style && typeof current.style === "object" ? current.style : {},
    };
  }

  function signatureCompletedItems(items) {
    return (Array.isArray(items) ? items : []).map((item) => ({
      kind: item.kind || "source",
      text: item.text || "",
      style_slot: item.style_slot || "",
    }));
  }

  function buildEmptyRenderSignature() {
    return JSON.stringify({
      preset: overlayState.preset,
      compact: overlayState.compact,
      completedItems: [],
      activePartialText: "",
      style: overlayState.lastPayloadStyle,
      rendered: [],
    });
  }

  function cancelPendingOverlayPayload() {
    pendingOverlayPayload = null;
    if (overlayRenderRafId) {
      window.cancelAnimationFrame(overlayRenderRafId);
      overlayRenderRafId = 0;
    }
    if (overlayRenderTimerId) {
      window.clearTimeout(overlayRenderTimerId);
      overlayRenderTimerId = 0;
    }
  }

  function flushScheduledOverlayPayload() {
    overlayRenderRafId = 0;
    overlayRenderTimerId = 0;
    lastOverlayRenderAt = (typeof performance !== "undefined" && performance && typeof performance.now === "function")
      ? performance.now()
      : Date.now();
    const next = pendingOverlayPayload;
    pendingOverlayPayload = null;
    if (next) {
      applyOverlayPayload(next);
    }
  }

  function scheduleOverlayPayload(payload) {
    pendingOverlayPayload = payload;
    if (overlayRenderRafId || overlayRenderTimerId) {
      return;
    }
    const minIntervalMs = overlayRenderMinIntervalMs(payload);
    const now = (typeof performance !== "undefined" && performance && typeof performance.now === "function")
      ? performance.now()
      : Date.now();
    const elapsed = lastOverlayRenderAt > 0 ? now - lastOverlayRenderAt : minIntervalMs;
    const delayMs = minIntervalMs > 0 && elapsed < minIntervalMs ? minIntervalMs - elapsed : 0;
    if (delayMs > 0) {
      overlayRenderTimerId = window.setTimeout(() => {
        overlayRenderTimerId = 0;
        overlayRenderRafId = window.requestAnimationFrame(flushScheduledOverlayPayload);
      }, delayMs);
      return;
    }
    overlayRenderRafId = window.requestAnimationFrame(flushScheduledOverlayPayload);
  }

  function writeDebug(message, details) {
    const timestamp = new Date().toLocaleTimeString();
    const suffix = details
      ? ` | ${typeof details === "string" ? details : JSON.stringify(details)}`
      : "";
    const line = `[${timestamp}] ${message}${suffix}`;
    if (debugMode) {
      console.debug(`[overlay] ${line}`);
    }
  }

  // Ring buffer for the last subtitle-effect trace events so a developer can
  // inspect window.__sstOverlaySubtitleTrace from DevTools without re-running
  // the session. The buffer caps at 200 entries — enough to capture a typical
  // utterance worth of partial frames without growing unbounded.
  const SUBTITLE_TRACE_RING_LIMIT = 200;
  const subtitleTraceRing = [];
  if (subtitleDebugMode) {
    window.__sstOverlaySubtitleTrace = subtitleTraceRing;
  }

  function handleSubtitleRenderTrace(event) {
    if (!subtitleDebugMode || !event || typeof event !== "object") {
      return;
    }
    const enriched = { ts: Date.now(), ...event };
    subtitleTraceRing.push(enriched);
    if (subtitleTraceRing.length > SUBTITLE_TRACE_RING_LIMIT) {
      subtitleTraceRing.splice(0, subtitleTraceRing.length - SUBTITLE_TRACE_RING_LIMIT);
    }
    if (event.type === "partial_frame") {
      console.debug(
        `[overlay-subtitles] partial slot=${event.slot} transition=${event.transition} `
          + `shared=${event.shared_length} fresh=${event.fresh_chars} prev_len=${event.previous_text_length} `
          + `cur_len=${event.current_text_length} effect=${event.effect}`
      );
    } else if (event.type === "completed_frame") {
      console.debug(
        `[overlay-subtitles] completed slot=${event.slot} animated=${event.animated} `
          + `text_len=${event.text_length} effect=${event.effect}`
      );
    } else if (event.type === "render_summary") {
      const anomalyTags = (event.anomalies || []).map((a) => a.kind).join(",") || "none";
      console.debug(
        `[overlay-subtitles] summary rows=${event.rows} partials=${event.partial_entries} `
          + `completed=${event.completed_entries} state_carryover=${event.state_carryover} `
          + `since_last_ms=${event.ms_since_last_render} duration_ms=${event.render_duration_ms.toFixed(2)} `
          + `anomalies=${anomalyTags}`
      );
    }
  }

  function applyClasses() {
    if (!root) {
      return;
    }
    root.className = `overlay ${overlayState.preset}${overlayState.compact ? " compact" : ""}`;
  }

  function hasRenderableOverlayContent(payload) {
    if (!payload || typeof payload !== "object") {
      return false;
    }
    const visibleItems = Array.isArray(payload.visible_items)
      ? payload.visible_items.filter((item) => String(item?.text || "").trim())
      : [];
    if (visibleItems.length > 0) {
      return true;
    }
    return Boolean(String(payload.active_partial_text || "").trim());
  }

  function isOverlayPresentationEmpty() {
    return (
      String(overlayState.lifecycleState || "idle") === "idle"
      && !String(overlayState.activePartialText || "").trim()
      && (!Array.isArray(overlayState.completedItems) || overlayState.completedItems.length === 0)
    );
  }

  function hasVisibleRenderedFrame() {
    if (!overlayState.lastRenderSignature) {
      return false;
    }
    return overlayState.lastRenderSignature !== buildEmptyRenderSignature();
  }

  function clearOverlayPresentation(reason) {
    // State may already be cleared (e.g. idle TTL path clears completedItems first),
    // but the renderer DOM can still show the last frame until render/dispose runs.
    if (isOverlayPresentationEmpty() && !hasVisibleRenderedFrame()) {
      return;
    }
    cancelPendingOverlayPayload();
    overlayState.completedItems = [];
    overlayState.activePartialText = "";
    overlayState.lifecycleState = "idle";
    if (reason) {
      writeDebug("text hidden", reason);
    }
    render();
  }

  async function isRuntimeReachable() {
    try {
      const response = await fetch("/live", { cache: "no-store" });
      return response.ok;
    } catch (_error) {
      return false;
    }
  }

  function scheduleRuntimeGoneClear(currentConnectionId) {
    window.setTimeout(() => {
      if (currentConnectionId !== connectionId) {
        return;
      }
      void isRuntimeReachable().then((alive) => {
        if (currentConnectionId !== connectionId || alive) {
          return;
        }
        clearOverlayPresentation("runtime unavailable");
      });
    }, runtimeGoneClearDelayMs);
  }

  function buildPresentationPayload() {
    const completedItems = overlayState.completedItems.map((item) => ({
      kind: item.kind || "source",
      text: item.text || "",
    }));
    return {
      preset: overlayState.preset,
      compact: overlayState.compact,
      completed_block_visible: completedItems.length > 0,
      visible_items: completedItems,
      active_partial_text: overlayState.activePartialText,
      // Forward the backend's lifecycle_state so composeRenderRows can
      // detect the "completed_with_partial" mix and mark the live partial
      // text (sitting inside visible_items[source]) as transient.
      // Without this, the renderer would treat every keystroke as a new
      // completed entry and re-render the whole source line each frame
      // while a translation is visible.
      lifecycle_state: overlayState.lifecycleState || "idle",
      show_source: overlayState.showSource,
      show_translations: overlayState.showTranslations,
      style: overlayState.lastPayloadStyle || {},
    };
  }

  function render() {
    const payload = buildPresentationPayload();
    const rows = window.SubtitleStyleRenderer
      ? window.SubtitleStyleRenderer.composeRenderRows(payload)
      : [];
    const renderedTexts = rows.flatMap((row) => row.entries || []).map((entry) => entry.text);
    const signature = JSON.stringify({
      preset: overlayState.preset,
      compact: overlayState.compact,
      completedItems: signatureCompletedItems(overlayState.completedItems),
      activePartialText: overlayState.activePartialText,
      style: overlayState.lastPayloadStyle,
      rendered: rows,
    });
    const emptySignature = buildEmptyRenderSignature();
    if (signature !== overlayState.lastRenderSignature) {
      const previousHadText = Boolean(
        overlayState.lastRenderSignature
        && overlayState.lastRenderSignature !== emptySignature
      );
      if (renderedTexts.length === 0 && previousHadText) {
        writeDebug("text hidden", "overlay became empty");
      } else if (renderedTexts.length > 0 && !previousHadText) {
        writeDebug("text shown", renderedTexts.join(" || "));
      } else if (renderedTexts.length > 0) {
        writeDebug("text updated", renderedTexts.join(" || "));
      }
      overlayState.lastRenderSignature = signature;
    } else {
      applyClasses();
      return;
    }
    if (window.SubtitleStyleRenderer && linesContainer) {
      const result = window.SubtitleStyleRenderer.render(linesContainer, payload, {
        overlay: true,
        onRenderTrace: subtitleDebugMode ? handleSubtitleRenderTrace : null,
      });
      // Match dashboard preview (`overlay-panel.js`): when TTL expiry, Stop, or
      // idle payload leaves no visible lines, tear down renderer state and DOM.
      // Without this, fast-path carry-over can leave the last subtitle frame
      // visible in OBS even after the backend sent an empty overlay_update.
      if (result?.empty) {
        window.SubtitleStyleRenderer.disposeRenderContainer(linesContainer);
      }
    } else if (linesContainer) {
      linesContainer.textContent = renderedTexts.join("\n");
    }
    applyClasses();
  }

  function applyOverlayPayload(rawPayload) {
    const payload = normalizeOverlayPayload(rawPayload);
    if (OVERLAY_PRESETS.has(payload.preset)) {
      overlayState.preset = payload.preset;
    }
    overlayState.compact = payload.compact;
    const visibleItems = payload.visible_items;
    const itemTexts = visibleItems.map((item) => item.text).filter(Boolean);
    overlayState.showSource = payload.show_source;
    overlayState.showTranslations = payload.show_translations;
    overlayState.lastPayloadStyle = payload.style;

    overlayState.activePartialText = overlayState.showSource
      ? payload.active_partial_text
      : "";
    // Capture the backend lifecycle so buildPresentationPayload() can pass
    // it through to the renderer's composeRenderRows — required for the
    // "completed_with_partial" transient-source classification.
    overlayState.lifecycleState = payload.lifecycle_state;
    writeDebug("overlay payload", JSON.stringify({
      state: payload.lifecycle_state,
      completed: Boolean(payload.completed_block_visible),
      partial: overlayState.activePartialText ? "yes" : "no",
      items: itemTexts.length,
      preset: payload.preset,
      show_source: overlayState.showSource,
      show_translations: overlayState.showTranslations,
      max_translation_languages: payload.max_translation_languages,
      display_order: payload.display_order,
      visible_texts: itemTexts,
    }));

    // SST TTL contract: while the next phrase is partial, the previous phrase's
    // completed translation block stays in visible_items with lifecycle
    // completed_with_partial. Only partial_only / idle clear the block.
    const lifecycleState = payload.lifecycle_state;
    const keepCompletedBlock =
      lifecycleState === "completed_with_partial"
      || lifecycleState === "completed_only"
      || (payload.completed_block_visible === true && itemTexts.length > 0);
    if (keepCompletedBlock && itemTexts.length > 0) {
      overlayState.completedItems = visibleItems
        .filter((item) => item && item.text)
        .map((item) => ({
          kind: item.kind || "source",
          text: item.text || "",
          style_slot: item.style_slot || "",
          lang: item.lang || "",
          slot_id: item.slot_id || "",
          target_lang: item.target_lang || "",
        }));
    } else {
      overlayState.completedItems = [];
    }
    if (lifecycleState === "idle" && !hasRenderableOverlayContent(payload)) {
      clearOverlayPresentation("idle empty overlay");
      return;
    }
    render();
  }

  function connect() {
    connectionId += 1;
    const currentConnectionId = connectionId;
    const protocol = location.protocol === "https:" ? "wss" : "ws";
    const ws = new WebSocket(`${protocol}://${location.host}/ws/events`);

    ws.addEventListener("open", () => {
      if (currentConnectionId !== connectionId) {
        try {
          ws.close();
        } catch (_error) {
          // ignore
        }
        return;
      }
      reconnectBackoffMs = 1000;
      writeDebug(
        "ws connected",
        `preset=${overlayState.preset}, compact=${overlayState.compact ? "on" : "off"}, debug=${debugMode ? "on" : "off"}`
      );
    });

    ws.addEventListener("message", (event) => {
      if (currentConnectionId !== connectionId) {
        return;
      }
      try {
        const data = JSON.parse(event.data);
        const eventType = normalizeWsEventType(data.type);
        if (eventType === "overlay_update" && data.payload) {
          if (isWsEventStale(staleGuard, eventType, data.payload)) {
            writeDebug("overlay payload", "ignored stale overlay_update");
            return;
          }
          scheduleOverlayPayload(data.payload);
        }
      } catch (_error) {
        // ignore malformed messages in skeleton stage
      }
    });

    ws.addEventListener("close", () => {
      if (currentConnectionId !== connectionId) {
        return;
      }
      writeDebug("ws disconnected", `keeping last frame; reconnect in ${reconnectBackoffMs}ms`);
      scheduleRuntimeGoneClear(currentConnectionId);
      if (reconnectTimer !== null) {
        window.clearTimeout(reconnectTimer);
      }
      reconnectTimer = window.setTimeout(() => {
        reconnectTimer = null;
        connect();
      }, reconnectBackoffMs);
      reconnectBackoffMs = Math.min(maxReconnectBackoffMs, reconnectBackoffMs * 2);
    });

    ws.addEventListener("error", () => {
      try {
        ws.close();
      } catch (_error) {
        // ignore
      }
    });
  }

  writeDebug(
    "overlay boot",
    `preset=${preset}, compact=${overlayState.compact ? "on" : "off"}`
      + (subtitleDebugMode ? `, subtitle_debug=on (source=${subtitleDebugFromUrl ? "url" : "localStorage"})` : "")
  );
  applyClasses();
  connect();
})();
