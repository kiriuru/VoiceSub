mod common;

use common::{
    assert_contains, assert_not_contains, count_innerhtml_wipe_statements, read_workspace_file,
    slice_from_function,
};

fn subtitle_style_js() -> String {
    read_workspace_file("bin/overlay/shared/js/subtitle-style.js")
}

#[test]
fn renderer_exports_common_prefix_length() {
    let source = subtitle_style_js();
    assert_contains(&source, "commonPrefixLength", "export");
    assert_contains(&source, "commonPrefixLength,", "public export list");
}

#[test]
fn renderer_emits_fragment_classes_for_partial_split() {
    let source = subtitle_style_js();
    assert_contains(&source, "subtitle-fragment-static", "partial fragments");
    assert_contains(&source, "subtitle-fragment-fresh", "partial fragments");
    assert_contains(&source, "appendTransientFragments", "partial fragments");
}

#[test]
fn renderer_reuses_surface_on_pure_extension() {
    let source = subtitle_style_js();
    assert_contains(
        &source,
        "updateTransientSurfaceInPlace",
        "partial flicker guard",
    );
    assert_contains(
        &source,
        "updateTransientSurfaceInPlace,",
        "public export list",
    );
    assert_contains(&source, "partialSurfaceBySlot", "partial surface cache");
}

#[test]
fn initial_partial_creates_empty_static_span_for_reuse() {
    let source = subtitle_style_js();
    let body = slice_from_function(&source, "appendTransientFragments", 2000);
    assert!(
        !body.contains("if (staticPart)"),
        "appendTransientFragments must not gate static span on non-empty staticPart"
    );
}

#[test]
fn renderer_marks_reused_surface_in_trace() {
    let source = subtitle_style_js();
    assert_contains(&source, "reused_surface", "debug trace");
    assert_contains(&source, "reused_partial_surfaces", "debug trace");
}

#[test]
fn pure_extension_check_uses_shared_prefix_invariant() {
    let source = subtitle_style_js();
    assert_contains(
        &source,
        "sharedLength === previousText.length",
        "pure extension guard",
    );
}

#[test]
fn renderer_exposes_partial_transition_classifier() {
    let source = subtitle_style_js();
    assert_contains(&source, "classifyPartialTransition", "export");
    assert_contains(&source, "classifyPartialTransition,", "public export list");
    for label in ["initial", "identical", "extension", "shrink", "revision", "jump"] {
        assert_contains(&source, &format!("\"{label}\""), "transition label");
    }
}

#[test]
fn renderer_has_shape_signature_fast_path() {
    let source = subtitle_style_js();
    for needle in [
        "function _shapeSignatureForEntry",
        "function _shapeSignatureForRows",
        "_shapeSignatureForRows,",
        "_shapeSignatureForEntry,",
        "shapeSignature",
        "entrySurfaces",
        "cachedWrapper.parentNode === container",
        "fast_path: true",
        "fast_path: false",
    ] {
        assert_contains(&source, needle, "shape fast path");
    }
}

#[test]
fn fast_path_keeps_single_innerhtml_wipe_statement() {
    let source = subtitle_style_js();
    assert_eq!(
        count_innerhtml_wipe_statements(&source),
        1,
        "subtitle-style.js must contain exactly one container.innerHTML wipe statement"
    );
}

#[test]
fn compose_render_rows_marks_completed_with_partial_source_as_transient() {
    let source = subtitle_style_js();
    assert_contains(&source, "livePartialSourceInVisibleItems", "composer");
    assert_contains(
        &source,
        "lifecycle_state === \"completed_with_partial\"",
        "lifecycle gate",
    );
    assert_contains(&source, "item.kind === \"source\"", "transient source kind");
    assert_contains(
        &source,
        "String(item.text || \"\") === activePartialText",
        "transient source text match",
    );
    assert_not_contains(
        &source,
        "item.kind === \"translation\" && livePartialSourceInVisibleItems",
        "translation transient guard",
    );
}

#[test]
fn compose_render_rows_skips_partial_only_shortcut_for_completed_with_partial() {
    let source = subtitle_style_js();
    assert_contains(&source, "isCompletedWithPartial", "composer");
    assert_contains(&source, "!isCompletedWithPartial", "partial-only shortcut guard");
    assert_contains(
        &source,
        "!payload?.completed_block_visible && !isCompletedWithPartial",
        "completed block guard",
    );
}

#[test]
fn renderer_emits_structured_debug_trace_events() {
    let source = subtitle_style_js();
    assert_contains(&source, "_resolveTraceCallback(options)", "trace callback");
    assert_contains(&source, "onRenderTrace", "trace hook");
    for event_type in ["partial_frame", "completed_frame", "render_summary"] {
        assert_contains(&source, &format!("\"{event_type}\""), "trace event");
    }
    assert_contains(&source, "\"partial_revision\"", "trace anomaly");
    assert_contains(&source, "\"state_carryover_missing\"", "trace anomaly");
    for field in [
        "rows: rows.length",
        "partial_entries: partialEntryCount",
        "completed_entries: completedEntryCount",
        "state_carryover: hadPriorRenderState",
        "ms_since_last_render",
        "render_duration_ms",
    ] {
        assert_contains(&source, field, "render summary");
    }
}

#[test]
fn renderer_persists_dom_refs_as_weakref_when_supported() {
    let source = subtitle_style_js();
    assert_contains(&source, "function _surfaceRefFor", "weakref helper");
    assert_contains(&source, "function _derefSurfaceRef", "weakref helper");
    assert_contains(&source, "typeof WeakRef === \"function\"", "weakref feature detect");
    assert_contains(
        &source,
        "_surfaceRefsFromElements(nextEntrySurfaces)",
        "entry surface refs",
    );
    assert_contains(
        &source,
        "_persistPartialSurfaceBySlot(nextPartialSurfaceBySlot)",
        "partial surface refs",
    );
    assert_contains(&source, "wrapper: _surfaceRefFor(wrapper)", "wrapper ref");
}

#[test]
fn slow_path_releases_orphaned_surfaces_before_innerhtml_wipe() {
    let source = subtitle_style_js();
    assert_contains(&source, "function _releaseOrphanedSurfaces", "orphan cleanup");
    assert_contains(
        &source,
        "const keepSurfaces = new Set(nextEntrySurfaces)",
        "orphan keep set",
    );
    assert_contains(
        &source,
        "_releaseOrphanedSurfaces(previousEntrySurfaces, keepSurfaces)",
        "orphan cleanup call",
    );
    assert_contains(&source, "delete surface.__sstAppliedStyleMap", "style map cleanup");
}

#[test]
fn slow_path_can_reuse_completed_source_surface() {
    let source = subtitle_style_js();
    assert_contains(&source, "reusableCompletedSurface", "slow path reuse");
    assert_contains(&source, "reused_completed_surface", "slow path trace");
    assert_contains(&source, "prev.transient === false", "completed-only reuse");
    assert_contains(&source, "prev.slot === slotName", "slot match");
    assert_contains(&source, "prev.kind === entryKind", "kind match");
    assert_contains(&source, "prev.lang === entryLang", "lang match");
    assert_contains(&source, "prev.text === entryText", "text match");
    assert_contains(
        &source,
        "previousPartialSurfaceBySlot.get(slotName)",
        "partial surface lookup",
    );
    assert_contains(
        &source,
        "lastPartialTextForSlot === entryText",
        "partial finalize text match",
    );
}

#[test]
fn renderer_exports_finalization_fast_path_helpers() {
    let source = subtitle_style_js();
    for needle in [
        "function _canFastPathFinalize",
        "function _finalizeTransientSurfaceInPlace",
        "_canFastPathFinalize,",
        "_finalizeTransientSurfaceInPlace,",
        "exactShapeMatch || finalizationCompatible",
        "entryDescriptors",
        "finalized_in_place",
    ] {
        assert_contains(&source, needle, "finalization fast path");
    }
    let helper_body = slice_from_function(&source, "_finalizeTransientSurfaceInPlace", 1200);
    assert_contains(&helper_body, "effect-none", "finalize helper");
    assert_contains(&helper_body, "animated: false", "finalize helper");
    assert_contains(
        &helper_body,
        "surface.removeChild(surface.firstChild)",
        "finalize helper",
    );
    assert_contains(&helper_body, "surface.textContent = text", "finalize helper");
}

#[test]
fn renderer_exports_dispose_for_panel_unmount() {
    let source = subtitle_style_js();
    assert_contains(&source, "disposeRenderContainer", "export");
    assert_contains(&source, "disposeRenderContainer,", "public export list");
    assert_contains(
        &source,
        "delete container.__subtitleStyleRenderState",
        "render state cleanup",
    );
}
