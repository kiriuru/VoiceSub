mod common;

use common::{assert_contains, read_workspace_file};

#[test]
fn overlay_forwards_lifecycle_state_into_render_payload() {
    let source = read_workspace_file("bin/overlay/overlay.js");
    assert_contains(&source, "lifecycleState", "overlay state");
    assert_contains(
        &source,
        "overlayState.lifecycleState = payload.lifecycle_state",
        "applyOverlayPayload",
    );
    assert_contains(
        &source,
        "lifecycle_state: overlayState.lifecycleState || \"idle\"",
        "buildPresentationPayload",
    );
}

#[test]
fn overlay_keeps_completed_block_for_completed_with_partial() {
    let source = read_workspace_file("bin/overlay/overlay.js");
    assert_contains(&source, "completed_with_partial", "lifecycle");
    assert_contains(
        &source,
        "lifecycleState === \"completed_with_partial\"",
        "completed block retention",
    );
    assert_contains(&source, "keepCompletedBlock", "completed block retention");
}

#[test]
fn dashboard_overlay_normalizer_preserves_lifecycle_state() {
    let source = read_workspace_file("src/lib/overlay-normalizer.ts");
    assert_contains(&source, "lifecycle_state", "overlay normalizer");
    for state in [
        "idle",
        "partial_only",
        "completed_only",
        "completed_with_partial",
    ] {
        assert_contains(&source, &format!("\"{state}\""), "lifecycle allowlist");
    }
    assert_contains(
        &source,
        "LIFECYCLE_STATES.has(rawLifecycle) ? rawLifecycle : \"idle\"",
        "lifecycle coercion",
    );
}

#[test]
fn overlay_skips_render_when_signature_unchanged() {
    let source = read_workspace_file("bin/overlay/overlay.js");
    assert_contains(
        &source,
        "signature !== overlayState.lastRenderSignature",
        "signature gate",
    );
    assert_contains(
        &source,
        "overlayState.lastRenderSignature = signature",
        "signature update",
    );
    assert_contains(&source, "} else {", "signature unchanged branch");
    assert_contains(
        &source,
        "signature !== overlayState.lastRenderSignature",
        "signature comparison before branch",
    );
    let else_pos = source
        .find("} else {")
        .expect("else branch after signature gate");
    let after_else = &source[else_pos..else_pos + 120];
    assert!(
        after_else.contains("applyClasses()") && after_else.contains("return;"),
        "skip render fast path: {after_else}"
    );
}

#[test]
fn overlay_ignores_stale_ws_payloads() {
    let source = read_workspace_file("bin/overlay/overlay.js");
    assert_contains(&source, "SstWsStaleGuard", "stale guard import");
    assert_contains(&source, "isWsEventStale(staleGuard", "stale guard usage");
    assert_contains(
        &source,
        "ignored stale overlay_update",
        "stale overlay_update trace",
    );
}

#[test]
fn overlay_disposes_renderer_when_payload_is_empty() {
    let source = read_workspace_file("bin/overlay/overlay.js");
    assert_contains(
        &source,
        "const result = window.SubtitleStyleRenderer.render",
        "render result",
    );
    assert_contains(
        &source,
        "window.SubtitleStyleRenderer.disposeRenderContainer(linesContainer)",
        "empty payload cleanup",
    );
    assert_contains(&source, "result?.empty", "empty result gate");
}

#[test]
fn overlay_clears_dom_when_idle_arrives_after_state_already_cleared() {
    let source = read_workspace_file("bin/overlay/overlay.js");
    assert_contains(&source, "hasVisibleRenderedFrame", "rendered frame probe");
    assert_contains(
        &source,
        "isOverlayPresentationEmpty() && !hasVisibleRenderedFrame()",
        "idle TTL must still tear down DOM",
    );
}

#[test]
fn overlay_subtitle_debug_hook_is_opt_in() {
    let source = read_workspace_file("bin/overlay/overlay.js");
    assert_contains(
        &source,
        "params.get(\"debug-subtitles\") === \"1\"",
        "debug url param",
    );
    assert_contains(&source, "\"sst_debug_subtitles\"", "debug storage key");
    assert_contains(&source, "subtitleDebugMode", "debug flag");
    assert_contains(
        &source,
        "onRenderTrace: subtitleDebugMode ? handleSubtitleRenderTrace : null",
        "renderer trace gate",
    );
    assert_contains(&source, "__sstOverlaySubtitleTrace", "devtools ring buffer");
}

#[test]
fn overlay_uses_public_live_probe_not_protected_health() {
    let source = read_workspace_file("bin/overlay/overlay.js");
    assert_contains(&source, "fetch(\"/live\"", "public liveness probe");
    assert!(
        !source.contains("/api/health"),
        "overlay must not call protected /api/health"
    );
}

#[test]
fn overlay_normalizes_lifecycle_state() {
    let source = read_workspace_file("bin/overlay/overlay.js");
    assert_contains(&source, "LIFECYCLE_STATES", "lifecycle allowlist");
    for state in [
        "idle",
        "partial_only",
        "completed_only",
        "completed_with_partial",
    ] {
        assert_contains(&source, &format!("\"{state}\""), "lifecycle allowlist");
    }
    assert_contains(&source, "normalizeOverlayPayload", "overlay payload normalizer");
    assert_contains(
        &source,
        "LIFECYCLE_STATES.has(rawLifecycle) ? rawLifecycle : \"idle\"",
        "lifecycle coercion",
    );
}

#[test]
fn overlay_uses_overlay_update_only() {
    let source = read_workspace_file("bin/overlay/overlay.js");
    assert_contains(&source, "overlay_update", "overlay_update handler");
    assert!(
        !source.contains("transcript_update"),
        "OBS overlay must not subscribe to legacy transcript_update"
    );
}

#[test]
fn overlay_signature_uses_renderable_completed_fields() {
    let source = read_workspace_file("bin/overlay/overlay.js");
    assert_contains(&source, "signatureCompletedItems", "renderable signature helper");
    assert_contains(&source, "buildEmptyRenderSignature", "empty signature helper");
}

#[test]
fn overlay_throttles_long_text_repaints() {
    let source = read_workspace_file("bin/overlay/overlay.js");
    assert_contains(
        &source,
        "OVERLAY_LONG_TEXT_MIN_RENDER_MS",
        "long text render interval",
    );
    assert_contains(
        &source,
        "overlayRenderMinIntervalMs",
        "adaptive overlay throttle",
    );
    assert_contains(
        &source,
        "OVERLAY_DENSE_PARTIAL_CHARS",
        "shared dense text threshold",
    );
}

#[test]
fn overlay_renderer_uses_append_only_partial_merge() {
    let source = read_workspace_file("bin/overlay/shared/js/subtitle-style.js");
    assert_contains(
        &source,
        "OVERLAY_DENSE_PARTIAL_CHARS",
        "dense partial threshold",
    );
    assert_contains(
        &source,
        "mergeFreshIntoStatic",
        "append-only partial merge",
    );
    assert_contains(
        &source,
        "resolveFreshFragmentEffect",
        "animation budget policy",
    );
    assert_contains(&source, "is-dense-partial", "dense partial css hook");
}

#[test]
fn overlay_does_not_post_http_logs() {
    let source = read_workspace_file("bin/overlay/overlay.js");
    assert!(
        !source.contains("/api/logs/"),
        "overlay must not POST client/ui logs over HTTP"
    );
    assert!(
        !source.contains("sendBeacon"),
        "overlay must not use sendBeacon log fallback"
    );
}
